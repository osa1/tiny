use libtiny_common::{ChanName, ChanNameRef};

use crate::config::*;
use crate::notifier::Notifier;

#[test]
fn parsing_tab_configs() {
    let config_str = r##"
        servers:
          - addr: "server"
            join: 
              - "#tiny -ignore -notify messages"
            notifier: mentions
          - addr: "server2"
            join:
              - "#tiny2" 
            ignore: true
        defaults:
            ignore: false
            notifier: off
        "##;
    let config: Config = serde_yaml::from_str(config_str).expect("parsed config");
    let expected = Config {
        servers: vec![
            Server {
                addr: "server".to_string(),
                join: vec![Chan {
                    name: ChanName::new("#tiny".to_string()),
                    config: TabConfig {
                        ignore: Some(true),
                        notifier: Some(Notifier::Messages),
                    },
                }],
                configs: TabConfig {
                    ignore: None,
                    notifier: Some(Notifier::Mentions),
                },
            },
            Server {
                addr: "server2".to_string(),
                join: vec![Chan {
                    name: ChanName::new("#tiny2".to_string()),
                    config: TabConfig {
                        ignore: None,
                        notifier: None,
                    },
                }],
                configs: TabConfig {
                    ignore: Some(true),
                    notifier: None,
                },
            },
        ],
        defaults: Defaults {
            tab_configs: TabConfig {
                ignore: Some(false),
                notifier: Some(Notifier::Off),
            },
        },
        ..Default::default()
    };
    assert_eq!(config.servers, expected.servers);
    assert_eq!(config.defaults, expected.defaults);

    assert_eq!(
        config.server_tab_configs("server"),
        TabConfig {
            ignore: Some(false),                // overwritten by defaults
            notifier: Some(Notifier::Mentions)  // configured
        }
    );

    assert_eq!(
        config.server_tab_configs("server2"),
        TabConfig {
            ignore: Some(true),            // configured
            notifier: Some(Notifier::Off)  // overwritten by defaults
        }
    );

    assert_eq!(
        config.server_tab_configs("randomserver"),
        TabConfig {
            ignore: Some(false),           // defaults
            notifier: Some(Notifier::Off)  // defaults
        }
    );

    assert_eq!(
        config.chan_tab_configs("server", ChanNameRef::new("#tiny")),
        TabConfig {
            ignore: Some(true),                 // configured
            notifier: Some(Notifier::Messages)  // configured
        }
    );

    assert_eq!(
        config.chan_tab_configs("server", ChanNameRef::new("##rust")),
        TabConfig {
            ignore: Some(false),                // overwritten by defaults
            notifier: Some(Notifier::Mentions)  // overwritten by server
        }
    );

    assert_eq!(
        config.chan_tab_configs("server2", ChanNameRef::new("#tiny2")),
        TabConfig {
            ignore: Some(true),            // overwritten by server
            notifier: Some(Notifier::Off)  // overwritten by defaults
        }
    );
}
