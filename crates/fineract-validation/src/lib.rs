// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements. See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership. The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License. You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. See the License for the
// specific language governing permissions and limitations
// under the License.

//! Rust migration of `org.apache.fineract.validation.constraints`.
//!
//! Provides constraint validators equivalent to Fineract's Jakarta Bean Validation constraints:
//! - [`DateFormatValidator`] (`@DateFormat`)
//! - [`EnumValueValidator`] (`@EnumValue`)
//! - [`LocalDateValidator`] (`@LocalDate`)
//! - [`LocaleValidator`] (`@Locale`)

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Default validation error messages from `ValidationMessages.properties`.
pub mod messages {
    pub const WRONG_LOCAL_DATE_FIELDS: &str = "Wrong local date fields.";
    pub const WRONG_LOCALE_FORMAT: &str = "Wrong locale format.";
    pub const INVALID_ENUM_VALUE: &str = "Invalid enum value.";
    pub const WRONG_DATE_FORMAT: &str = "Wrong date format.";
}

/// Represents a constraint violation produced by a validator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintViolation {
    pub property_path: String,
    pub message: String,
    pub message_template: String,
}

impl ConstraintViolation {
    pub fn new(
        property_path: impl Into<String>,
        message: impl Into<String>,
        message_template: impl Into<String>,
    ) -> Self {
        Self {
            property_path: property_path.into(),
            message: message.into(),
            message_template: message_template.into(),
        }
    }
}

/// Configuration error when a `@LocalDate` validator references non-existent fields.
#[derive(Debug, Error)]
#[error("Invalid configuration for @LocalDate: field '{0}' not found")]
pub struct LocalDateConfigurationError(pub String);

// ============================================================================
// 1. DateFormatValidator (`org.apache.fineract.validation.constraints.DateFormatValidator`)
// ============================================================================

/// Validator for `@DateFormat` constraint.
#[derive(Debug, Default, Clone, Copy)]
pub struct DateFormatValidator;

impl DateFormatValidator {
    pub fn new() -> Self {
        Self
    }

    /// Checks whether an optional value is valid according to `@DateFormat`.
    /// Blank values are allowed (`true`); use a non-blank check separately if required.
    pub fn is_valid(&self, value: Option<&str>) -> bool {
        match value {
            None => true,
            Some(s) if s.trim().is_empty() => true,
            Some(s) => Self::is_valid_pattern(s),
        }
    }

    /// Checks whether the given string is a valid Java `DateTimeFormatter` pattern.
    /// Can be used by validators that are not annotation-based (e.g. when validating JSON commands).
    pub fn is_valid_pattern(pattern: &str) -> bool {
        if pattern.trim().is_empty() {
            return true;
        }
        validate_java_datetime_pattern(pattern)
    }

    /// Validates a field value and returns any constraint violations.
    pub fn validate_field(
        &self,
        field_name: &str,
        value: Option<&str>,
    ) -> Vec<ConstraintViolation> {
        if self.is_valid(value) {
            Vec::new()
        } else {
            vec![ConstraintViolation::new(
                field_name,
                messages::WRONG_DATE_FORMAT,
                "{org.apache.fineract.validation.constraints.DateFormat.message}",
            )]
        }
    }
}

/// Validates a pattern string against Java `java.time.format.DateTimeFormatterBuilder` rules.
fn validate_java_datetime_pattern(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut has_pattern_letter = false;

    while i < len {
        let ch = chars[i];
        if ch == '\'' {
            // Quoted literal
            i += 1;
            let mut closed = false;
            while i < len {
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        i += 2;
                        continue;
                    } else {
                        closed = true;
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            if !closed {
                // Unclosed quote is illegal in DateTimeFormatter.ofPattern
                return false;
            }
        } else if ch.is_ascii_alphabetic() {
            // Count contiguous run of identical pattern letters
            let start = i;
            while i < len && chars[i] == ch {
                i += 1;
            }
            let count = i - start;
            if !is_valid_java_pattern_letter(ch, count) {
                return false;
            }
            has_pattern_letter = true;
        } else if ch.is_ascii_digit() {
            // Unquoted digits (e.g. "02 February 2026") are not valid pattern syntax
            return false;
        } else if matches!(ch, '{' | '}' | '#') {
            // Reserved characters in Java DateTimeFormatterBuilder
            return false;
        } else {
            // Punctuation/separators like '-', '/', ' ', ',', ':', '.'
            i += 1;
        }
    }

    has_pattern_letter
}

fn is_valid_java_pattern_letter(ch: char, count: usize) -> bool {
    match ch {
        // Era
        'G' => (1..=5).contains(&count),
        // Year
        'u' | 'y' | 'Y' => count >= 1,
        // Quarter
        'Q' | 'q' => (1..=5).contains(&count),
        // Month
        'M' | 'L' => (1..=5).contains(&count),
        // Week
        'w' => (1..=2).contains(&count),
        'W' => count == 1,
        // Day
        'd' => (1..=2).contains(&count),
        'D' => (1..=3).contains(&count),
        'F' => count == 1,
        'g' => count >= 1,
        // Day of week
        'E' | 'e' | 'c' => (1..=5).contains(&count),
        // AM/PM
        'a' => count == 1,
        // Hour / Minute / Second
        'h' | 'K' | 'k' | 'H' | 'm' | 's' => (1..=2).contains(&count),
        // Fraction of second / Nano / Milli
        'S' => (1..=9).contains(&count),
        'A' | 'n' | 'N' => count >= 1,
        // Timezone
        'V' => count == 2,
        'v' => count == 1 || count == 4,
        'z' => (1..=4).contains(&count),
        'O' => count == 1 || count == 4,
        'X' | 'x' | 'Z' => (1..=5).contains(&count),
        // Pad
        'p' => count >= 1,
        // Any other letter (such as 'b', 'f', 'i', 'o', 'r', 't', etc.) is invalid
        _ => false,
    }
}

// ============================================================================
// 2. EnumValueValidator (`org.apache.fineract.validation.constraints.EnumValueValidator`)
// ============================================================================

/// Trait implemented by enums that can be validated with [`EnumValueValidator`].
pub trait EnumVariants {
    fn variant_names() -> &'static [&'static str];
}

/// Validator for `@EnumValue` constraint.
#[derive(Debug, Clone)]
pub struct EnumValueValidator {
    accepted_values: HashSet<String>,
}

impl EnumValueValidator {
    /// Initializes the validator from an explicit slice of enum constant names.
    pub fn new<I, S>(variants: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let accepted_values = variants
            .into_iter()
            .map(|s| s.as_ref().to_ascii_lowercase())
            .collect();
        Self { accepted_values }
    }

    /// Initializes the validator from a type implementing [`EnumVariants`].
    pub fn for_enum<E: EnumVariants>() -> Self {
        Self::new(E::variant_names())
    }

    /// Checks whether the candidate string matches one of the enum variants (case-insensitive, root locale).
    pub fn is_valid(&self, value: Option<&str>) -> bool {
        match value {
            Some(v) => self.accepted_values.contains(&v.to_ascii_lowercase()),
            None => false,
        }
    }

    /// Validates a field value and returns any constraint violations.
    pub fn validate_field(
        &self,
        field_name: &str,
        value: Option<&str>,
    ) -> Vec<ConstraintViolation> {
        if self.is_valid(value) {
            Vec::new()
        } else {
            vec![ConstraintViolation::new(
                field_name,
                messages::INVALID_ENUM_VALUE,
                "{org.apache.fineract.validation.constraints.EnumValue.message}",
            )]
        }
    }
}

// ============================================================================
// 3. LocaleValidator (`org.apache.fineract.validation.constraints.LocaleValidator`)
// ============================================================================

/// Validator for `@Locale` constraint.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocaleValidator;

impl LocaleValidator {
    pub fn new() -> Self {
        Self
    }

    /// Checks whether the given string is a valid available locale tag (e.g., `"en"`, `"EN"`, `"en-US"`, `"en_US"`).
    pub fn is_valid(&self, value: Option<&str>) -> bool {
        let Some(raw) = value else {
            return false;
        };
        if raw.trim().is_empty() {
            return false;
        }
        // Reject leading/trailing whitespace or internal spaces
        if raw != raw.trim() || raw.contains(char::is_whitespace) {
            return false;
        }

        // Normalize input to use BCP 47 format (e.g., "en-US")
        let language_tag = raw.replace('_', "-");
        is_available_bcp47_locale(&language_tag)
    }

    /// Validates a field value and returns any constraint violations.
    pub fn validate_field(
        &self,
        field_name: &str,
        value: Option<&str>,
    ) -> Vec<ConstraintViolation> {
        if self.is_valid(value) {
            Vec::new()
        } else {
            vec![ConstraintViolation::new(
                field_name,
                messages::WRONG_LOCALE_FORMAT,
                "{org.apache.fineract.validation.constraints.Locale.message}",
            )]
        }
    }
}

/// Checks if a normalized BCP-47 language tag matches Java's available ISO/CLDR locales.
fn is_available_bcp47_locale(tag: &str) -> bool {
    // Available ISO 639-1 language codes supported by standard JDK Locale.getAvailableLocales()
    const VALID_LANGUAGES: &[&str] = &[
        "af", "agq", "ak", "am", "ar", "as", "asa", "ast", "az", "bas", "be", "bem", "bez", "bg",
        "bm", "bn", "bo", "br", "brx", "bs", "ca", "ccp", "ce", "ceb", "cgg", "chr", "ckb", "cs",
        "cy", "da", "dav", "de", "dje", "dsb", "dua", "dyo", "dz", "ebu", "ee", "el", "en", "eo",
        "es", "et", "eu", "ewo", "fa", "ff", "fi", "fil", "fo", "fr", "fur", "fy", "ga", "gd",
        "gl", "gsw", "gu", "guz", "gv", "ha", "haw", "he", "hi", "hr", "hsb", "hu", "hy", "ia",
        "id", "ig", "ii", "is", "it", "ja", "jgo", "jmc", "jv", "ka", "kab", "kam", "kde", "kea",
        "khq", "ki", "kk", "kkj", "kl", "kln", "km", "kn", "ko", "kok", "ks", "ksb", "ksf", "ksh",
        "ku", "kw", "ky", "lag", "lb", "lg", "lkt", "ln", "lo", "lrc", "lt", "lu", "luo", "luy",
        "lv", "mas", "mer", "mfe", "mg", "mgh", "mgo", "mi", "mk", "ml", "mn", "mr", "ms", "mt",
        "mua", "my", "mzn", "naq", "nb", "nd", "nds", "ne", "nl", "nmg", "nn", "nnh", "no", "nus",
        "nyn", "om", "or", "os", "pa", "pcm", "pl", "ps", "pt", "qu", "rm", "rn", "ro", "rof",
        "ru", "rw", "rwk", "sah", "saq", "sbp", "sd", "se", "seh", "ses", "sg", "shi", "si", "sk",
        "sl", "smn", "sn", "so", "sq", "sr", "sv", "sw", "ta", "te", "teo", "tg", "th", "ti", "tk",
        "to", "tr", "tt", "twq", "tzm", "ug", "uk", "ur", "uz", "vai", "vi", "vun", "wae", "wo",
        "xh", "xog", "yav", "yi", "yo", "yue", "zgh", "zh", "zu",
    ];

    const VALID_REGIONS: &[&str] = &[
        "AD", "AE", "AF", "AG", "AI", "AL", "AM", "AO", "AR", "AS", "AT", "AU", "AW", "AX", "AZ",
        "BA", "BB", "BD", "BE", "BF", "BG", "BH", "BI", "BJ", "BL", "BM", "BN", "BO", "BQ", "BR",
        "BS", "BT", "BW", "BY", "BZ", "CA", "CC", "CD", "CF", "CG", "CH", "CI", "CK", "CL", "CM",
        "CN", "CO", "CR", "CU", "CV", "CW", "CX", "CY", "CZ", "DE", "DG", "DJ", "DK", "DM", "DO",
        "DZ", "EA", "EC", "EE", "EG", "ER", "ES", "ET", "FI", "FJ", "FK", "FM", "FO", "FR", "GA",
        "GB", "GD", "GE", "GF", "GG", "GH", "GI", "GL", "GM", "GN", "GP", "GQ", "GR", "GT", "GU",
        "GW", "GY", "HK", "HN", "HR", "HT", "HU", "IC", "ID", "IE", "IL", "IM", "IN", "IO", "IQ",
        "IR", "IS", "IT", "JE", "JM", "JO", "JP", "KE", "KG", "KH", "KI", "KM", "KN", "KP", "KR",
        "KW", "KY", "KZ", "LA", "LB", "LC", "LI", "LK", "LR", "LS", "LT", "LU", "LV", "LY", "MA",
        "MC", "MD", "ME", "MF", "MG", "MH", "MK", "ML", "MM", "MN", "MO", "MP", "MQ", "MR", "MS",
        "MT", "MU", "MV", "MW", "MX", "MY", "MZ", "NA", "NC", "NE", "NF", "NG", "NI", "NL", "NO",
        "NP", "NR", "NU", "NZ", "OM", "PA", "PE", "PF", "PG", "PH", "PK", "PL", "PM", "PN", "PR",
        "PS", "PT", "PW", "PY", "QA", "RE", "RO", "RS", "RU", "RW", "SA", "SB", "SC", "SD", "SE",
        "SG", "SH", "SI", "SJ", "SK", "SL", "SM", "SN", "SO", "SR", "SS", "ST", "SV", "SX", "SY",
        "SZ", "TC", "TD", "TG", "TH", "TJ", "TK", "TL", "TM", "TN", "TO", "TR", "TT", "TV", "TW",
        "TZ", "UA", "UG", "UM", "US", "UY", "UZ", "VA", "VC", "VE", "VG", "VI", "VN", "VU", "WF",
        "WS", "XK", "YE", "YT", "ZA", "ZM", "ZW",
    ];

    let segments: Vec<&str> = tag.split('-').collect();
    match segments.as_slice() {
        [lang] => {
            let lang_lower = lang.to_ascii_lowercase();
            VALID_LANGUAGES.binary_search(&lang_lower.as_str()).is_ok()
        }
        [lang, region] => {
            let lang_lower = lang.to_ascii_lowercase();
            let region_upper = region.to_ascii_uppercase();
            VALID_LANGUAGES.binary_search(&lang_lower.as_str()).is_ok()
                && VALID_REGIONS.binary_search(&region_upper.as_str()).is_ok()
        }
        _ => false,
    }
}

// ============================================================================
// 4. LocalDateValidator (`org.apache.fineract.validation.constraints.LocalDateValidator`)
// ============================================================================

/// Trait for models that expose date, format, and locale fields by name.
pub trait HasNamedFields {
    fn get_field(&self, name: &str) -> Result<Option<&str>, LocalDateConfigurationError>;
}

impl HasNamedFields for HashMap<String, Option<String>> {
    fn get_field(&self, name: &str) -> Result<Option<&str>, LocalDateConfigurationError> {
        match self.get(name) {
            Some(opt) => Ok(opt.as_deref()),
            None => Err(LocalDateConfigurationError(name.to_string())),
        }
    }
}

/// Validator for `@LocalDate` class-level constraint.
#[derive(Debug, Clone)]
pub struct LocalDateValidator {
    pub date_field: String,
    pub format_field: String,
    pub locale_field: String,
}

impl LocalDateValidator {
    pub fn new(
        date_field: impl Into<String>,
        format_field: impl Into<String>,
        locale_field: impl Into<String>,
    ) -> Self {
        Self {
            date_field: date_field.into(),
            format_field: format_field.into(),
            locale_field: locale_field.into(),
        }
    }

    /// Validates an object implementing [`HasNamedFields`].
    /// Returns `Err(LocalDateConfigurationError)` if any configured field name does not exist on the target type.
    pub fn is_valid<T: HasNamedFields>(
        &self,
        value: &T,
    ) -> Result<bool, LocalDateConfigurationError> {
        let date = value.get_field(&self.date_field)?;
        let format = value.get_field(&self.format_field)?;
        let locale = value.get_field(&self.locale_field)?;

        Ok(Self::is_valid_fields(date, format, locale))
    }

    /// Directly validates `(date, format, locale)` string triples.
    pub fn is_valid_fields(
        date: Option<&str>,
        format: Option<&str>,
        locale: Option<&str>,
    ) -> bool {
        let (Some(date_str), Some(format_str), Some(locale_str)) = (date, format, locale) else {
            return false;
        };
        if date_str.trim().is_empty()
            || format_str.trim().is_empty()
            || locale_str.trim().is_empty()
        {
            return false;
        }
        if !LocaleValidator.is_valid(Some(locale_str)) {
            return false;
        }
        if !DateFormatValidator::is_valid_pattern(format_str) {
            return false;
        }
        parse_strict_local_date(date_str, format_str).is_some()
    }

    /// Validates a model and returns any [`ConstraintViolation`]s.
    pub fn validate<T: HasNamedFields>(
        &self,
        value: &T,
    ) -> Result<Vec<ConstraintViolation>, LocalDateConfigurationError> {
        if self.is_valid(value)? {
            Ok(Vec::new())
        } else {
            Ok(vec![ConstraintViolation::new(
                "",
                messages::WRONG_LOCAL_DATE_FIELDS,
                "{org.apache.fineract.validation.constraints.LocalDate.message}",
            )])
        }
    }
}

/// Parses a date string against a Java `DateTimeFormatter` pattern with `ResolverStyle.STRICT`
/// and optional ` HH:mm:ss` suffix, matching `LocalDateValidator.toLocalDate`.
pub fn parse_strict_local_date(date_str: &str, java_pattern: &str) -> Option<NaiveDate> {
    // Strip optional " HH:mm:ss" time suffix if present
    let date_part = if date_str.len() > 9 {
        let potential_time = &date_str[date_str.len() - 9..];
        if potential_time.starts_with(' ') && is_valid_hh_mm_ss(&potential_time[1..]) {
            &date_str[..date_str.len() - 9]
        } else {
            date_str
        }
    } else {
        date_str
    };

    // Convert Java pattern tokens (e.g. dd, MM, MMM, MMMM, yyyy, uuuu) into a token stream
    let tokens = tokenize_java_date_pattern(java_pattern)?;
    match_date_tokens(date_part, &tokens)
}

fn is_valid_hh_mm_ss(s: &str) -> bool {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        return false;
    }
    let Ok(h) = parts[0].parse::<u32>() else {
        return false;
    };
    let Ok(m) = parts[1].parse::<u32>() else {
        return false;
    };
    let Ok(sec) = parts[2].parse::<u32>() else {
        return false;
    };
    p_len_2(&parts) && h < 24 && m < 60 && sec < 60
}

fn p_len_2(parts: &[&str]) -> bool {
    parts.iter().all(|p| p.len() == 2)
}

#[derive(Debug)]
enum PatternToken {
    Year(usize),
    MonthNumber(usize),
    MonthText,
    Day(usize),
    Literal(String),
}

fn tokenize_java_date_pattern(pattern: &str) -> Option<Vec<PatternToken>> {
    let normalized = pattern.replace('y', "u");
    let chars: Vec<char> = normalized.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut tokens = Vec::new();

    while i < len {
        let ch = chars[i];
        if ch == '\'' {
            i += 1;
            let mut lit = String::new();
            let mut closed = false;
            while i < len {
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        lit.push('\'');
                        i += 2;
                        continue;
                    } else {
                        closed = true;
                        i += 1;
                        break;
                    }
                }
                lit.push(chars[i]);
                i += 1;
            }
            if !closed {
                return None;
            }
            tokens.push(PatternToken::Literal(lit));
        } else if ch.is_ascii_alphabetic() {
            let start = i;
            while i < len && chars[i] == ch {
                i += 1;
            }
            let count = i - start;
            match ch {
                'u' | 'Y' => tokens.push(PatternToken::Year(count)),
                'M' | 'L' => {
                    if count <= 2 {
                        tokens.push(PatternToken::MonthNumber(count));
                    } else if count <= 5 {
                        tokens.push(PatternToken::MonthText);
                    } else {
                        return None;
                    }
                }
                'd' => {
                    if count <= 2 {
                        tokens.push(PatternToken::Day(count));
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        } else {
            let start = i;
            while i < len && !chars[i].is_ascii_alphabetic() && chars[i] != '\'' {
                i += 1;
            }
            let lit: String = chars[start..i].iter().collect();
            tokens.push(PatternToken::Literal(lit));
        }
    }
    Some(tokens)
}

fn match_date_tokens(input: &str, tokens: &[PatternToken]) -> Option<NaiveDate> {
    let mut rest = input;
    let mut year: Option<i32> = None;
    let mut month: Option<u32> = None;
    let mut day: Option<u32> = None;

    for token in tokens {
        match token {
            PatternToken::Literal(lit) => {
                if rest.len() < lit.len() || !rest[..lit.len()].eq_ignore_ascii_case(lit) {
                    return None;
                }
                rest = &rest[lit.len()..];
            }
            PatternToken::Year(count) => {
                let digits_len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
                if digits_len == 0 || (*count == 4 && digits_len != 4) {
                    return None;
                }
                let y_str = &rest[..digits_len];
                let y = y_str.parse::<i32>().ok()?;
                year = Some(if *count == 2 && digits_len == 2 {
                    2000 + y
                } else {
                    y
                });
                rest = &rest[digits_len..];
            }
            PatternToken::MonthNumber(_count) => {
                let digits_len = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .take(2)
                    .count();
                if digits_len == 0 {
                    return None;
                }
                let m = rest[..digits_len].parse::<u32>().ok()?;
                if !(1..=12).contains(&m) {
                    return None;
                }
                month = Some(m);
                rest = &rest[digits_len..];
            }
            PatternToken::MonthText => {
                let alpha_len: usize = rest
                    .chars()
                    .take_while(|c| c.is_alphabetic() || *c == '.')
                    .map(|c| c.len_utf8())
                    .sum();
                if alpha_len == 0 {
                    return None;
                }
                let m_str = &rest[..alpha_len];
                let m = parse_month_name(m_str)?;
                month = Some(m);
                rest = &rest[alpha_len..];
            }
            PatternToken::Day(_count) => {
                let digits_len = rest
                    .chars()
                    .take_while(|c| c.is_ascii_digit())
                    .take(2)
                    .count();
                if digits_len == 0 {
                    return None;
                }
                let d = rest[..digits_len].parse::<u32>().ok()?;
                if !(1..=31).contains(&d) {
                    return None;
                }
                day = Some(d);
                rest = &rest[digits_len..];
            }
        }
    }

    if !rest.is_empty() {
        return None;
    }

    NaiveDate::from_ymd_opt(year?, month?, day?)
}

fn parse_month_name(name: &str) -> Option<u32> {
    let lower = name.trim_end_matches('.').to_ascii_lowercase();
    match lower.as_str() {
        "jan" | "january" | "januar" | "ene" | "enero" | "janv" | "janvier" => Some(1),
        "feb" | "february" | "februar" | "febrero" | "fevr" | "février" => Some(2),
        "mar" | "march" | "märz" | "maerz" | "marzo" | "mars" => Some(3),
        "apr" | "april" | "abr" | "abril" | "avr" | "avril" => Some(4),
        "may" | "mai" | "mayo" => Some(5),
        "jun" | "june" | "juni" | "junio" | "juin" => Some(6),
        "jul" | "july" | "juli" | "julio" | "juil" | "juillet" => Some(7),
        "aug" | "august" | "ago" | "agosto" | "août" | "aout" => Some(8),
        "sep" | "sept" | "september" | "septiembre" | "septembre" => Some(9),
        "oct" | "october" | "okt" | "oktober" | "octubre" | "octobre" => Some(10),
        "nov" | "november" | "noviembre" | "novembre" => Some(11),
        "dec" | "december" | "dez" | "dezember" | "dic" | "diciembre" | "déc" | "décembre" => {
            Some(12)
        }
        _ => None,
    }
}

// ============================================================================
// Tests (Migrated from fineract-validation/src/test/java/...)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------------------
    // DateFormatValidationTest
    // ------------------------------------------------------------------------

    #[derive(Debug, Default)]
    struct DateFormatModel {
        date_format: Option<String>,
    }

    impl DateFormatModel {
        fn new(date_format: impl Into<String>) -> Self {
            Self {
                date_format: Some(date_format.into()),
            }
        }

        fn validate(&self) -> Vec<ConstraintViolation> {
            DateFormatValidator::new().validate_field("dateFormat", self.date_format.as_deref())
        }
    }

    #[test]
    fn date_format_blank_is_valid() {
        let request = DateFormatModel::new("");
        let errors = request.validate();
        assert!(errors.is_empty());
    }

    #[test]
    fn date_format_invalid_patterns() {
        for date_format in [
            "02 February 2026", // literal date, not a pattern
            "invalid",
            "dd bbb yyyy", // 'b' is not a valid pattern letter
        ] {
            let request = DateFormatModel::new(date_format);
            let errors = request.validate();
            assert_eq!(
                errors.len(),
                1,
                "Expected dateFormat '{}' to be invalid",
                date_format
            );
            assert!(errors.iter().any(|e| e.property_path == "dateFormat"));
        }
    }

    #[test]
    fn date_format_valid_patterns() {
        for date_format in ["dd MMMM yyyy", "yyyy-MM-dd", "dd/MM/yyyy", "MMM dd, yyyy"] {
            let request = DateFormatModel::new(date_format);
            let errors = request.validate();
            assert!(
                errors.is_empty(),
                "Expected dateFormat '{}' to be valid",
                date_format
            );
        }
    }

    #[test]
    fn static_is_valid_pattern_invalid() {
        assert!(!DateFormatValidator::is_valid_pattern("02 February 2026"));
        assert!(!DateFormatValidator::is_valid_pattern("unknown"));
    }

    #[test]
    fn static_is_valid_pattern_valid() {
        assert!(DateFormatValidator::is_valid_pattern("dd MMMM yyyy"));
        assert!(DateFormatValidator::is_valid_pattern("yyyy-MM-dd"));
    }

    // ------------------------------------------------------------------------
    // LocalDateValidationTest
    // ------------------------------------------------------------------------

    #[derive(Debug, Default)]
    struct LocalDateModel {
        date: Option<String>,
        format: Option<String>,
        locale: Option<String>,
    }

    impl HasNamedFields for LocalDateModel {
        fn get_field(&self, name: &str) -> Result<Option<&str>, LocalDateConfigurationError> {
            match name {
                "date" => Ok(self.date.as_deref()),
                "format" => Ok(self.format.as_deref()),
                "locale" => Ok(self.locale.as_deref()),
                other => Err(LocalDateConfigurationError(other.to_string())),
            }
        }
    }

    impl LocalDateModel {
        fn validate(&self) -> Vec<ConstraintViolation> {
            let validator = LocalDateValidator::new("date", "format", "locale");
            validator.validate(self).expect("valid field configuration")
        }
    }

    #[test]
    fn local_date_invalid_all_blank() {
        let request = LocalDateModel {
            format: Some("".to_string()),
            date: Some("  ".to_string()),
            locale: None,
        };
        let errors = request.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors
            .iter()
            .all(|e| e.message == "Wrong local date fields."));
    }

    #[test]
    fn local_date_invalid_locale_format() {
        let request = LocalDateModel {
            format: Some("dd-MM-yyyy".to_string()),
            date: Some("12-05-2025".to_string()),
            locale: Some("".to_string()),
        };
        let errors = request.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors
            .iter()
            .all(|e| e.message == "Wrong local date fields."));
    }

    #[test]
    fn local_date_invalid_date_format() {
        let request = LocalDateModel {
            format: Some("dd/MM/yyyy".to_string()),
            date: Some("12-05-2025".to_string()),
            locale: Some("en".to_string()),
        };
        let errors = request.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors
            .iter()
            .all(|e| e.message == "Wrong local date fields."));
    }

    #[test]
    fn local_date_valid() {
        let request = LocalDateModel {
            format: Some("dd-MM-yyyy".to_string()),
            date: Some("12-05-2025".to_string()),
            locale: Some("en".to_string()),
        };
        let errors = request.validate();
        assert!(errors.is_empty());
    }

    #[test]
    fn local_date_with_month_name_and_strict_leap_year() {
        let valid_leap = LocalDateModel {
            format: Some("dd MMMM yyyy".to_string()),
            date: Some("29 February 2024".to_string()),
            locale: Some("en".to_string()),
        };
        assert!(valid_leap.validate().is_empty());

        let invalid_non_leap = LocalDateModel {
            format: Some("dd MMMM yyyy".to_string()),
            date: Some("29 February 2025".to_string()),
            locale: Some("en".to_string()),
        };
        assert_eq!(invalid_non_leap.validate().len(), 1);
    }

    // ------------------------------------------------------------------------
    // LocaleValidationTest
    // ------------------------------------------------------------------------

    #[derive(Debug, Default)]
    struct LocaleModel {
        locale: Option<String>,
    }

    impl LocaleModel {
        fn validate(&self) -> Vec<ConstraintViolation> {
            LocaleValidator::new().validate_field("locale", self.locale.as_deref())
        }
    }

    #[test]
    fn locale_invalid_blank() {
        let request = LocaleModel { locale: None };
        let errors = request.validate();
        assert_eq!(errors.len(), 1);
        assert!(errors.iter().any(|e| e.property_path == "locale"));
    }

    #[test]
    fn locale_invalid_formats() {
        for locale in [
            "invalid-locale", // invalid format
            "xx-YY",          // non-existent locale
            "random text",    // random text
            "123",            // numbers
            "en-US-extra",    // extra segment
        ] {
            let request = LocaleModel {
                locale: Some(locale.to_string()),
            };
            let errors = request.validate();
            assert_eq!(
                errors.len(),
                1,
                "Expected locale '{}' to be invalid but it was valid",
                locale
            );
        }
    }

    #[test]
    fn locale_valid_locales() {
        for locale in [
            "en",    // language only
            "EN",    // uppercase language only
            "en-US", // language with country (hyphen)
            "en_US", // language with country (underscore)
        ] {
            let request = LocaleModel {
                locale: Some(locale.to_string()),
            };
            let errors = request.validate();
            assert!(
                errors.is_empty(),
                "Expected locale '{}' to be valid but it was invalid",
                locale
            );
        }
    }

    // ------------------------------------------------------------------------
    // EnumValueValidator Test
    // ------------------------------------------------------------------------

    #[allow(dead_code)]
    enum SampleFrequency {
        Days,
        Weeks,
        Months,
        Years,
    }

    impl EnumVariants for SampleFrequency {
        fn variant_names() -> &'static [&'static str] {
            &["DAYS", "WEEKS", "MONTHS", "YEARS"]
        }
    }

    #[test]
    fn enum_value_validator_works() {
        let validator = EnumValueValidator::for_enum::<SampleFrequency>();
        assert!(validator.is_valid(Some("MONTHS")));
        assert!(validator.is_valid(Some("months")));
        assert!(validator.is_valid(Some("Months")));
        assert!(!validator.is_valid(Some("CENTURIES")));
        assert!(!validator.is_valid(None));
    }
}
