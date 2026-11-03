use regex::Regex;

/// Extract directory names from an HTML directory listing.
pub fn parse_directory_listing(html: &str) -> Vec<String> {
    let re = Regex::new(r#"href="([^"]+)/""#).unwrap();
    re.captures_iter(html)
        .filter_map(|c| {
            let href = &c[1];
            // Links to other sites are not entries in the listing.
            if href.contains("://") {
                return None;
            }
            let name = href.rsplit('/').next().unwrap_or(href);
            match name {
                "" | "." | ".." => None,
                name => Some(name.to_string()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_directory_listing() {
        let html = r#"
<html><body>
<a href="../">Parent Directory</a>
<a href="M1075-L/">M1075-L/</a>
<a href="P1375/">P1375/</a>
</body></html>
"#;
        let dirs = parse_directory_listing(html);
        assert_eq!(dirs, vec!["M1075-L", "P1375"]);
    }

    #[test]
    fn test_parse_directory_listing_with_absolute_hrefs() {
        let html = r#"
<html><body>
<a href="https://help.axis.com/en-us/axis-m1075-l/">Help</a>
<a href="../">Parent Directory</a>
<a href="/ftp/pub_soft/MPQT/M1075-L/">M1075-L/</a>
<a href="/ftp/pub_soft/MPQT/P1375/">P1375/</a>
<a href="/ftp/pub_soft/MPQT/howtoupgrade.txt">howtoupgrade.txt</a>
</body></html>
"#;
        let dirs = parse_directory_listing(html);
        assert_eq!(dirs, vec!["M1075-L", "P1375"]);
    }
}
