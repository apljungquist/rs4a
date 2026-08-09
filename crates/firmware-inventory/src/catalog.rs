use anyhow::Context;
use log::debug;
use quick_xml::{
    encoding::Decoder,
    events::{BytesStart, Event},
    Reader,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Model name, as the enclosing `<boxmodel>` spells it.
    pub product: String,
    /// Dotted version string.
    pub revision: String,
    /// Path to the fimage, relative to the software root.
    pub fileurl: String,
}

fn attribute(tag: &BytesStart, decoder: Decoder, key: &str) -> anyhow::Result<Option<String>> {
    let Some(attr) = tag
        .try_get_attribute(key)
        .with_context(|| format!("Failed to parse the {key} attribute"))?
    else {
        return Ok(None);
    };
    let value = attr
        .decode_and_unescape_value(decoder)
        .with_context(|| format!("Failed to decode the {key} attribute"))?
        .into_owned();
    Ok(Some(value))
}

pub fn parse_catalog(xml: &str) -> anyhow::Result<Vec<CatalogEntry>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut buf = Vec::new();
    // Every `<software>` is a child of the `<boxmodel>` naming the product it is firmware for.
    let mut product = None;
    loop {
        buf.clear();
        let event = reader
            .read_event_into(&mut buf)
            .context("Failed to parse catalog XML")?;
        match event {
            Event::Eof => break,
            Event::Start(tag) if tag.name().as_ref() == b"boxmodel" => {
                product = attribute(&tag, reader.decoder(), "name")?;
                if product.is_none() {
                    debug!("Skipping a <boxmodel> element with no name attribute");
                }
            }
            Event::End(tag) if tag.name().as_ref() == b"boxmodel" => {
                product = None;
            }
            Event::Start(tag) | Event::Empty(tag) if tag.name().as_ref() == b"software" => {
                let decoder = reader.decoder();
                let Some(fileurl) = attribute(&tag, decoder, "fileurl")? else {
                    debug!("Skipping <software> element with no fileurl attribute");
                    continue;
                };
                let Some(revision) = attribute(&tag, decoder, "revision")? else {
                    debug!("Skipping <software fileurl={fileurl:?}> with no revision attribute");
                    continue;
                };
                let Some(product) = product.clone() else {
                    debug!("Skipping <software fileurl={fileurl:?}> with no named <boxmodel>");
                    continue;
                };
                entries.push(CatalogEntry {
                    product,
                    revision,
                    fileurl,
                });
            }
            _ => {}
        }
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_catalog_attributes_software_to_the_model_it_is_nested_in() {
        let entries = parse_catalog(
            r#"
<document>
    <boxtype name="CameraServer">
        <boxmodel name="AXIS P8815-2">
            <software fileurl="/MPQT/P8815-2/10_6_0/P8815-2_10_6_0.bin" revision="10.6.0"/>
        </boxmodel>
        <boxmodel name="AXIS P8815-2 3D People Counter">
            <software fileurl="/MPQT/P8815-2_3D_People_Counter/11_11_192/P8815-2_3D_People_Counter_11_11_192.bin" revision="11.11.192"/>
        </boxmodel>
    </boxtype>
</document>
"#,
        )
        .unwrap();
        assert_eq!(
            entries,
            [
                CatalogEntry {
                    product: "AXIS P8815-2".to_string(),
                    revision: "10.6.0".to_string(),
                    fileurl: "/MPQT/P8815-2/10_6_0/P8815-2_10_6_0.bin".to_string(),
                },
                CatalogEntry {
                    product: "AXIS P8815-2 3D People Counter".to_string(),
                    revision: "11.11.192".to_string(),
                    fileurl: "/MPQT/P8815-2_3D_People_Counter/11_11_192/P8815-2_3D_People_Counter_11_11_192.bin".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_catalog_keeps_models_that_share_a_directory_apart() {
        // The whole P12 Mk II line publishes out of one directory, so the file url cannot say which
        // product a fimage is for, but the model it is nested in can.
        let entries = parse_catalog(
            r#"
<document>
    <boxtype name="CameraServer">
        <boxmodel name="AXIS P1265">
            <software fileurl="/MPQT/P12_MkII/12_9_57/P12_MkII_12_9_57.bin" revision="12.9.57"/>
        </boxmodel>
        <boxmodel name="AXIS P1275">
            <software fileurl="/MPQT/P12_MkII/12_9_57/P12_MkII_12_9_57.bin" revision="12.9.57"/>
        </boxmodel>
    </boxtype>
</document>
"#,
        )
        .unwrap();
        let products: Vec<_> = entries.iter().map(|e| e.product.as_str()).collect();
        assert_eq!(products, ["AXIS P1265", "AXIS P1275"]);
    }

    #[test]
    fn parse_catalog_skips_software_it_cannot_place() {
        let entries = parse_catalog(
            r#"
<document>
    <boxtype name="CameraServer">
        <boxmodel name="AXIS M1075-L">
            <software fileurl="/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin"/>
            <software revision="12.9.57"/>
        </boxmodel>
    </boxtype>
    <software fileurl="/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin" revision="12.9.57"/>
</document>
"#,
        )
        .unwrap();
        assert_eq!(entries, []);
    }
}
