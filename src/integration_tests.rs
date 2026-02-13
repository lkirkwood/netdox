use crate::{
    config::{LocalConfig, PluginStage},
    process,
    remote::RemoteInterface,
    update,
};

#[ignore]
#[tokio::test]
async fn test_full_integration() {
    let cfg = LocalConfig::read().unwrap();

    let write_only_results = update::run_plugin_stage(&cfg, PluginStage::WriteOnly, None, false)
        .await
        .unwrap();

    assert!(write_only_results.iter().all(|res| res.code == Some(0)));

    let read_write_results = update::run_plugin_stage(&cfg, PluginStage::ReadWrite, None, false)
        .await
        .unwrap();

    assert!(read_write_results.iter().all(|res| res.code == Some(0)));

    let connector_results = update::run_plugin_stage(&cfg, PluginStage::Connectors, None, false)
        .await
        .unwrap();

    assert!(connector_results.iter().all(|res| res.code == Some(0)));

    process(&cfg).await.unwrap();

    let con = cfg.con().await.unwrap();
    cfg.remote.publish(con, None).await.unwrap();
}
