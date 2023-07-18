use libtiny_common::{ChanName, ChanNameRef};

use crate::config::*;
use crate::notifier::Notifier;

#[test]
fn parsing_tab_configs() {
    let config_str = r##"
        servers:
          - addr: "server"
            join: 
              - name: "#tiny"
                ignore: true
                notify: "messages"
            notify: "mentions"
          - addr: "server2"
            join:
              - "#tiny2" 
            ignore: true
        defaults:
            ignore: false
            notify: off
        "##;
    let config: Config = serde_yaml::from_str(config_str).expect("parsed config");
    let tab_configs: TabConfigs = (&config).into();
    let expected = Config {
        servers: vec![
            Server {
                addr: "server".to_string(),
                join: vec![Chan::WithConfig {
                    name: ChanName::new("#tiny".to_string()),
                    config: TabConfig {
                        ignore: Some(true),
                        notify: Some(Notifier::Messages),
                    },
                }],
                config: TabConfig {
                    notify: Some(Notifier::Mentions),
                    ..Default::default()
                },
            },
            Server {
                addr: "server2".to_string(),
                join: vec![Chan::Name(ChanName::new("#tiny2".to_string()))],
                config: TabConfig {
                    ignore: Some(true),
                    ..Default::default()
                },
            },
        ],
        defaults: Defaults {
            tab_config: TabConfig {
                ignore: Some(false),
                notify: Some(Notifier::Off),
            },
        },
        ..Default::default()
    };
    assert_eq!(config.servers, expected.servers);
    assert_eq!(config.defaults, expected.defaults);

    assert_eq!(
        tab_configs.get("server", None),
        Some(TabConfig {
            ignore: Some(false),              // overwritten by defaults
            notify: Some(Notifier::Mentions)  // configured
        })
    );

    assert_eq!(
        tab_configs.get("server2", None),
        Some(TabConfig {
            ignore: Some(true),          // configured
            notify: Some(Notifier::Off)  // overwritten by defaults
        })
    );

    assert_eq!(tab_configs.get("randomserver", None), None);

    assert_eq!(
        tab_configs.get("server", Some(ChanNameRef::new("#tiny"))),
        Some(TabConfig {
            ignore: Some(true),               // configured
            notify: Some(Notifier::Messages)  // configured
        })
    );

    assert_eq!(
        tab_configs.get("server", Some(ChanNameRef::new("##rust"))),
        None
    );

    assert_eq!(
        tab_configs.get("server2", Some(ChanNameRef::new("#tiny2"))),
        Some(TabConfig {
            ignore: Some(true),          // overwritten by server
            notify: Some(Notifier::Off)  // overwritten by defaults
        })
    );
}
