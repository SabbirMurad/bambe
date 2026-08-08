use regex::Regex;

/***************************** REGULAR EXPRESSION *****************************/

#[allow(dead_code)]
pub const PASSWORD_REGEX: &str = r"^[!@#$%&*[:word:]\S]{6, 64}$";
#[allow(dead_code)]
pub const PHONE_NUMBER_REGEX: &str = r"^[\+\s\(\)\-[:digit:]]{10, 25}$";
#[allow(dead_code)]
pub const EMAIL_REGEX: &str = r"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})";

/* Remove in development if not needed */

pub fn validate_email(email: &str) -> Result<(), String> {
  let email_regex = Regex::new(EMAIL_REGEX).unwrap();

  if !email_regex.is_match(email) {
    Err("Invalid email".to_string())
  }
  else {
    Ok(())
  }
}

pub fn _validate_phone(phone: &str) -> Result<(), String> {
  let phone_regex = Regex::new(PHONE_NUMBER_REGEX).unwrap();

  if !phone_regex.is_match(phone) {
    Err("Invalid phone number".to_string())
  }
  else {
    Ok(())
  }
}

pub fn validate_password(password: &str, confirm_password: &str) -> Result<(), String> {
  let password_regex = Regex::new(PASSWORD_REGEX).unwrap();

  if password.len() < 6 {
    return Err("Password needs to be more then 6 letters".to_string());
  }

  if password.len() > 64 {
    return Err("Password needs to be less then 64 letters".to_string());
  }

  if password != confirm_password {
    return Err("Passwords does not match".to_string());
  }

  if !password_regex.is_match(password) {
      return Err(
          "only _,!,@,#,$,%,&,* and letters A-Z,a-z and numbers 0-9 is acceptable in password".to_string()
      );
  }

  Ok(())
}

pub fn validate_username(username: &str) -> Result<(), String> {
  let parts = username.split(" ");
  let collection: Vec<&str> = parts.collect();
  
  if collection.len() > 1 {
    return Err("Username can't contain any space".to_string());
  }

  if username.len() < 6 {
    return Err("Username must be 6 characters or more".to_string());
  }

  if username.len() > 32 {
    return Err("Username must be within 64 characters".to_string());
  }

  if !username.chars().all(|c| c.is_lowercase() || c.is_digit(10)) {
    return Err("Username must contain only lowercase letters and numbers".to_string());
  }

  Ok(())
}

pub fn validate_full_name(full_name: &str) -> Result<(), String> {
  if full_name.len() < 6 {
    return Err("Full Name must be 6 characters or more".to_string());
  }
  else if full_name.len() > 128 {
    return Err("Full Name must be within 128 characters".to_string());
  }
  else {
    Ok(())
  }
}

pub fn _validate_residential_address(residential_address: &str) -> Result<(), String> {
  if residential_address.len() < 12 {
    return Err("Residential Address must be 12 characters or more".to_string());
  }
  else if residential_address.len() > 256 {
    return Err("Residential Address must be within 256 characters".to_string());
  }
  else {
    Ok(())
  }
}

fn _check_for_profanity(input_text: &str) -> Result<&str, &str> {
  let banned_words = [
    "fuck", "boobs", "ass", "sex", "shit", "bitch", "dick",
    "pussy", "dildo", "asshole", "cunt", "faggot"
  ];

  for word in &banned_words {
    if input_text.contains(word) {
      return Err("Input contains a banned word.");
    }
  }

  Ok(input_text)
}
