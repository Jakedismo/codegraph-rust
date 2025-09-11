use codegraph_lb::*;
use std::sync::Arc;

fn sample_pool() -> EndpointPool {
    let cfg = PoolConfig {
        endpoints: vec![
            EndpointConfig {
                uri: "http://localhost:3001".into(),
                weight: 1,
                health_check_path: None,
            },
            EndpointConfig {
                uri: "http://localhost:3002".into(),
                weight: 3,
                health_check_path: None,
            },
        ],
    };
    EndpointPool::from_config(&cfg).unwrap()
}

#[test]
fn rr_picks_some_endpoint() {
    let pool = Arc::new(sample_pool());
    let rr = RoundRobin::new();
    let e = rr.pick(&pool, None).unwrap();
    assert!(e.is_healthy());
}

#[test]
fn hrw_sticky_key() {
    let pool = Arc::new(sample_pool());
    let hrw = HrwHashing::new();
    let a = hrw.pick(&pool, Some(b"user:1")).unwrap().id;
    let b = hrw.pick(&pool, Some(b"user:1")).unwrap().id;
    assert_eq!(a.0, b.0);
}
