use amd_apcb::Error;
use amd_host_image_builder_config::{
    SerdeBhdDirectoryVariant, SerdeBhdSource, SerdeConfig,
};
use std::path::Path;

#[test]
fn test_token_versioning() {
    let configuration_filename =
        Path::new("tests").join("data").join("Milan.efs.json5");
    let configuration_str =
        std::fs::read_to_string(configuration_filename).unwrap();
    let configuration: SerdeConfig =
        json5::from_str(&configuration_str).unwrap();
    match configuration.bhd {
        SerdeBhdDirectoryVariant::BhdDirectory(directory) => {
            let mut found_match = false;
            for entry in directory.entries {
                match entry.source {
                    SerdeBhdSource::Implied => {}
                    SerdeBhdSource::BlobFile(_) => {}
                    SerdeBhdSource::SecondLevelDirectory(_) => {}
                    SerdeBhdSource::ApcbJson(apcb) => {
                        found_match = true;
                        apcb.validate(None).unwrap();
                        apcb.validate(Some(0x42)).unwrap();
                    }
                }
            }
            assert!(found_match);
        }
        _ => panic!("test input is unexpected. Please update test"),
    }
}

#[test]
fn test_token_versioning_failure() {
    let configuration_filename =
        Path::new("tests").join("data").join("Milan-new.efs.json5");
    let configuration_str =
        std::fs::read_to_string(configuration_filename).unwrap();
    let configuration: SerdeConfig =
        json5::from_str(&configuration_str).unwrap();
    match configuration.bhd {
        SerdeBhdDirectoryVariant::BhdDirectory(directory) => {
            let mut found_match = false;
            for entry in directory.entries {
                match entry.source {
                    SerdeBhdSource::Implied => {}
                    SerdeBhdSource::BlobFile(_) => {}
                    SerdeBhdSource::SecondLevelDirectory(_) => {}
                    SerdeBhdSource::ApcbJson(apcb) => {
                        found_match = true;
                        apcb.validate(None).unwrap();
                        match apcb.validate(Some(0x42)) {
                            Err(Error::TokenVersionMismatch { .. }) => {}
                            Err(x) => {
                                panic!(
                                    "test failed with unexpected error {:?}",
                                    x
                                )
                            }
                            Ok(_) => {
                                panic!("test unexpectedly succeeded")
                            }
                        }
                    }
                }
            }
            assert!(found_match);
        }
        _ => panic!("test input is unexpected. Please update test"),
    }
}

#[test]
fn test_token_versioning_both_new() {
    let configuration_filename =
        Path::new("tests").join("data").join("Milan-new.efs.json5");
    let configuration_str =
        std::fs::read_to_string(configuration_filename).unwrap();
    let configuration: SerdeConfig =
        json5::from_str(&configuration_str).unwrap();
    match configuration.bhd {
        SerdeBhdDirectoryVariant::BhdDirectory(directory) => {
            let mut found_match = false;
            for entry in directory.entries {
                match entry.source {
                    SerdeBhdSource::Implied => {}
                    SerdeBhdSource::BlobFile(_) => {}
                    SerdeBhdSource::SecondLevelDirectory(_) => {}
                    SerdeBhdSource::ApcbJson(apcb) => {
                        found_match = true;
                        apcb.validate(None).unwrap();
                        match apcb.validate(Some(0x1004_5012)) {
                            Err(x) => {
                                panic!(
                                    "test failed with unexpected error {:?}",
                                    x,
                                )
                            }
                            Ok(_) => {}
                        }
                    }
                }
            }
            assert!(found_match);
        }
        _ => panic!("test input is unexpected. Please update test"),
    }
}
