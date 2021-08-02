use libtiny_common::ChanNameRef;

use crate::config::*;
use crate::notifier::Notifier;

#[test]
fn parsing_tab_configs() {
    let config_str = r##"
        servers:
          - addr: "server"
            join: 
              - name: "#tiny"
                ignore: false
                notifier: messages
            notifier: mentions
          - addr: "server2"
            join:
              - "#tiny2" 
        defaults:
            ignore: true
            notifier: off
        "##;
    let config: Config = serde_yaml::from_str(config_str).expect("parsed config");
    let expected = Config {
        servers: vec![
            Server {
                addr: "server".to_string(),
                join: vec![Chan::WithConfigs {
                    name: "#tiny".to_string(),
                    configs: TabConfigs {
                        ignore: Some(false),
                        notifier: Some(Notifier::Messages),
                    },
                }],
                configs: TabConfigs {
                    ignore: None,
                    notifier: Some(Notifier::Mentions),
                },
            },
            Server {
                addr: "server2".to_string(),
                join: vec![Chan::Name("#tiny2".to_string())],
                configs: TabConfigs {
                    ignore: None,
                    notifier: None,
                },
            },
        ],
        defaults: Defaults {
            tab_configs: TabConfigs {
                ignore: Some(true),
                notifier: Some(Notifier::Off),
            },
        },
        ..Default::default()
    };
    assert_eq!(config.servers, expected.servers);
    assert_eq!(config.defaults, expected.defaults);

    assert_eq!(
        config.server_tab_configs("server"),
        TabConfigs {
            ignore: Some(true),                 // overwritten by defaults
            notifier: Some(Notifier::Mentions)  // configured
        }
    );

    assert_eq!(
        config.server_tab_configs("server2"),
        TabConfigs {
            ignore: Some(true),            // overwritten by defaults
            notifier: Some(Notifier::Off)  // overwritten by defaults
        }
    );

    assert_eq!(
        config.server_tab_configs("randomserver"),
        TabConfigs {
            ignore: Some(true),            // defaults
            notifier: Some(Notifier::Off)  // defaults
        }
    );

    assert_eq!(
        config.chan_tab_configs("server", ChanNameRef::new("#tiny")),
        TabConfigs {
            ignore: Some(false),                // configured
            notifier: Some(Notifier::Messages)  // configured
        }
    );

    assert_eq!(
        config.chan_tab_configs("server", ChanNameRef::new("##rust")),
        TabConfigs {
            ignore: Some(true),                 // overwritten by defaults
            notifier: Some(Notifier::Mentions)  // overwritten by server
        }
    );

    assert_eq!(
        config.chan_tab_configs("server2", ChanNameRef::new("#tiny2")),
        TabConfigs {
            ignore: Some(true),            // overwritten by defaults
            notifier: Some(Notifier::Off)  // overwritten by defaults
        }
    );
}
