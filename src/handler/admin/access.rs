use chrono::Utc;
use uuid::Uuid;
use serde_json::json;
use mongodb::bson::doc;
use mongodb::Database;
use actix_web::{Error, HttpRequest};
use crate::BuiltIns::mongo::MongoDB;
use crate::Middleware::Auth::{require_access, AccessRequirement, User};
use crate::model::Account::{AccountRole, AdminAccount};
use crate::model::Store::{StaffMember, StaffRole, StaffStatus};

/*
  What an admin endpoint needs the caller's staff role to allow. Every
  handler under /api/v1/admin names one; `View` is the floor every active
  staff member has.
*/
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Permission {
    // Read anything in the console.
    View,
    // Products, variants, stock, categories, brands.
    Catalogue,
    // Order status and tracking.
    Fulfilment,
    // Money back to a customer.
    Refunds,
    Returns,
    Reviews,
    Coupons,
    // Editing customers and their loyalty rewards.
    Customers,
    Support,
    // Delivery methods, packaging, shipping zones.
    Shipping,
    // Store details, theme, currency.
    StoreSettings,
    // Who is on staff, and their roles.
    Staff,
    // Deleting a customer's data for good.
    AccountDeletion,
}

impl Permission {
    fn describe(self) -> &'static str {
        match self {
            Permission::View => "view the console",
            Permission::Catalogue => "change products, categories or brands",
            Permission::Fulfilment => "update orders",
            Permission::Refunds => "refund orders",
            Permission::Returns => "resolve returns",
            Permission::Reviews => "moderate reviews",
            Permission::Coupons => "change coupons",
            Permission::Customers => "change customers",
            Permission::Support => "answer support tickets",
            Permission::Shipping => "change delivery, packaging or shipping zones",
            Permission::StoreSettings => "change store settings",
            Permission::Staff => "manage staff",
            Permission::AccountDeletion => "delete customer accounts",
        }
    }
}

/*
  The four roles. Owner does everything; Manager runs the shop but not its
  settings or its staff; a Support agent works orders, returns and tickets
  but moves no money and changes no catalogue; an Analyst only reads.
*/
pub fn allows(role: &StaffRole, permission: Permission) -> bool {
    use Permission::*;
    match role {
        StaffRole::Owner => true,
        StaffRole::Manager => !matches!(permission, StoreSettings | Staff | AccountDeletion),
        StaffRole::SupportAgent => matches!(permission, View | Fulfilment | Returns | Support),
        StaffRole::Analyst => permission == View,
    }
}

/// Every permission the role has, for the console to hide what it can't use.
pub fn granted(role: &StaffRole) -> Vec<&'static str> {
    use Permission::*;
    [
        (View, "view"), (Catalogue, "catalogue"), (Fulfilment, "fulfilment"), (Refunds, "refunds"),
        (Returns, "returns"), (Reviews, "reviews"), (Coupons, "coupons"), (Customers, "customers"),
        (Support, "support"), (Shipping, "shipping"), (StoreSettings, "store_settings"),
        (Staff, "staff"), (AccountDeletion, "account_deletion"),
    ]
    .into_iter()
    .filter(|(p, _)| allows(role, *p))
    .map(|(_, name)| name)
    .collect()
}

fn forbidden(message: String) -> Error {
    actix_web::error::ErrorForbidden(json!({ "error": message }))
}

/*
  The caller's staff record, as the console should treat it right now.

  A store with no active Owner would have nobody able to manage staff, so
  whoever signs in to such a store becomes its Owner — which is also how the
  very first admin gets the role. Otherwise an admin not on the staff list,
  or suspended, gets nothing; a Pending one becomes Active on first use.
*/
pub async fn staff_member(db: &Database, admin_id: &str) -> Result<StaffMember, Error> {
    let col = db.collection::<StaffMember>("staff_member");
    let now = Utc::now().timestamp_millis();
    let mine = col.find_one(doc! { "admin_id": admin_id }).await.ok().flatten();

    let has_owner = col
        .count_documents(doc! { "role": "Owner", "status": { "$ne": "Suspended" } })
        .await
        .unwrap_or(0) > 0;
    if !has_owner {
        match &mine {
            Some(m) => {
                col.update_one(
                    doc! { "uuid": &m.uuid },
                    doc! { "$set": { "role": "Owner", "status": "Active", "joined_at": m.joined_at.unwrap_or(now) } },
                ).await.ok();
            }
            None => {
                let account = db.collection::<AdminAccount>("admin_account").find_one(doc! { "uuid": admin_id }).await.ok().flatten();
                let (name, email) = account
                    .map(|a| (if a.name.is_empty() { a.email_address.clone() } else { a.name }, a.email_address))
                    .unwrap_or_else(|| ("Admin".to_string(), String::new()));
                let owner = StaffMember {
                    uuid: Uuid::now_v7().to_string(),
                    admin_id: admin_id.to_string(),
                    name,
                    email,
                    role: StaffRole::Owner,
                    status: StaffStatus::Active,
                    avatar_image_id: None,
                    last_active_at: Some(now),
                    invited_by: None,
                    invited_at: now,
                    joined_at: Some(now),
                };
                col.insert_one(&owner).await.ok();
            }
        }
        log::info!("staff: {} became Owner — the store had none", admin_id);
        return col
            .find_one(doc! { "admin_id": admin_id })
            .await
            .ok()
            .flatten()
            .ok_or_else(|| forbidden("Your staff record couldn't be created. Try again.".into()));
    }

    let member = mine.ok_or_else(|| forbidden(
        "Your admin account isn't on the staff list. Ask the store's Owner to add you.".into(),
    ))?;
    match member.status {
        StaffStatus::Suspended => Err(forbidden("Your staff access has been suspended.".into())),
        StaffStatus::Pending => {
            col.update_one(
                doc! { "uuid": &member.uuid },
                doc! { "$set": { "status": "Active", "joined_at": now, "last_active_at": now } },
            ).await.ok();
            Ok(StaffMember { status: StaffStatus::Active, joined_at: Some(now), ..member })
        }
        StaffStatus::Active => {
            // Coarse on purpose: the Staff table shows "active 3 min ago",
            // not every request.
            if member.last_active_at.map(|t| now - t > 5 * 60 * 1000).unwrap_or(true) {
                col.update_one(doc! { "uuid": &member.uuid }, doc! { "$set": { "last_active_at": now } }).await.ok();
            }
            Ok(member)
        }
    }
}

/*
  The check every admin endpoint makes: a valid admin token, an active
  staff record, and a role that allows `permission`. Returns the caller,
  as `require_access` did, so handlers keep using `admin.user_id`.
*/
pub async fn require_staff(req: &HttpRequest, permission: Permission) -> Result<User, Error> {
    let admin = require_access(req, AccessRequirement::Role(AccountRole::Administrator))?;
    let member = staff_member(&MongoDB.connect(), &admin.user_id).await?;
    if !allows(&member.role, permission) {
        return Err(forbidden(format!(
            "Your role ({}) can't {}.",
            role_label(&member.role),
            permission.describe(),
        )));
    }
    Ok(admin)
}

pub fn role_label(role: &StaffRole) -> &'static str {
    match role {
        StaffRole::Owner => "Owner",
        StaffRole::Manager => "Manager",
        StaffRole::SupportAgent => "Support agent",
        StaffRole::Analyst => "Analyst",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Permission::*;

    const ALL: [Permission; 13] = [
        View, Catalogue, Fulfilment, Refunds, Returns, Reviews, Coupons,
        Customers, Support, Shipping, StoreSettings, Staff, AccountDeletion,
    ];

    #[test]
    fn owner_can_do_everything() {
        assert!(ALL.iter().all(|p| allows(&StaffRole::Owner, *p)));
    }

    #[test]
    fn manager_runs_the_shop_but_not_its_settings_or_staff() {
        for p in ALL {
            let expected = !matches!(p, StoreSettings | Staff | AccountDeletion);
            assert_eq!(allows(&StaffRole::Manager, p), expected, "{:?}", p);
        }
    }

    #[test]
    fn support_agent_works_orders_returns_and_tickets_only() {
        for p in ALL {
            let expected = matches!(p, View | Fulfilment | Returns | Support);
            assert_eq!(allows(&StaffRole::SupportAgent, p), expected, "{:?}", p);
        }
    }

    #[test]
    fn analyst_only_reads() {
        for p in ALL {
            assert_eq!(allows(&StaffRole::Analyst, p), p == View, "{:?}", p);
        }
    }

    #[test]
    fn granted_matches_allows() {
        assert_eq!(granted(&StaffRole::Analyst), vec!["view"]);
        assert_eq!(granted(&StaffRole::Owner).len(), ALL.len());
    }
}
