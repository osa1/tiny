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
    let tab_configs: TabConfigs = (&config).into();
    let expected = Config {
        servers: vec![
            Server {
                addr: "server".to_string(),
                join: vec![Chan {
                    name: ChanName::new("#tiny".to_string()),
                    config: Some(TabConfig {
                        ignore: true,
                        notifier: Notifier::Messages,
                    }),
                }],
                config: Some(TabConfig {
                    notifier: Notifier::Mentions,
                    ..Default::default()
                }),
            },
            Server {
                addr: "server2".to_string(),
                join: vec![Chan {
                    name: ChanName::new("#tiny2".to_string()),
                    config: None,
                }],
                config: Some(TabConfig {
                    ignore: true,
                    ..Default::default()
                }),
            },
        ],
        defaults: Defaults {
            tab_config: TabConfig {
                ignore: false,
                notifier: Notifier::Off,
            },
        },
        ..Default::default()
    };
    assert_eq!(config.servers, expected.servers);
    assert_eq!(config.defaults, expected.defaults);

    assert_eq!(
        tab_configs.serv_conf("server"),
        Some(TabConfig {
            ignore: false,                // overwritten by defaults
            notifier: Notifier::Mentions  // configured
        })
    );

    assert_eq!(
        tab_configs.serv_conf("server2"),
        Some(TabConfig {
            ignore: true,            // configured
            notifier: Notifier::Off  // overwritten by defaults
        })
    );

    assert_eq!(tab_configs.serv_conf("randomserver"), None);

    assert_eq!(
        tab_configs.chan_conf("server", ChanNameRef::new("#tiny")),
        Some(&TabConfig {
            ignore: true,                 // configured
            notifier: Notifier::Messages  // configured
        })
    );

    assert_eq!(
        tab_configs.chan_conf("server", ChanNameRef::new("##rust")),
        None
    );

    assert_eq!(
        tab_configs.chan_conf("server2", ChanNameRef::new("#tiny2")),
        Some(&TabConfig {
            ignore: true,            // overwritten by server
            notifier: Notifier::Off  // overwritten by defaults
        })
    );
}
