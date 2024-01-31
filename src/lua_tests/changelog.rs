use crate::{
    data::model::{CHANGELOG_KEY, DNS_KEY, PDATA_KEY, REPORTS_KEY},
    tests_common::*,
};
use redis::{streams::StreamRangeReply, AsyncCommands, Value};

// OBJECTS

#[tokio::test]
async fn test_changelog_create_dns() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let change = "create dns name";
    let qname = format!("[{DEFAULT_NETWORK}]changelog-dns-{}.com", *TIMESTAMP);

    call_fn(&mut con, function, &["1", &qname, PLUGIN]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_qname)) => {
                id_change == change.as_bytes() && id_qname == qname.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_create_node() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node";
    let change = "create plugin node";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-create-node-{}.com",
        *TIMESTAMP
    );

    call_fn(&mut con, function, &["1", &qname, PLUGIN]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_qname)) => {
                id_change == change.as_bytes() && id_qname == qname.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_create_report() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report";
    let change = "create report";
    let report = format!("changelog-create-report-{}", *TIMESTAMP);

    call_fn(&mut con, function, &["1", &report, PLUGIN, "title", "0"]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_report)) => {
                id_change == change.as_bytes() && id_report == report.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// NO CREATE OBJECTS

#[tokio::test]
async fn test_changelog_no_create_dns() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let change = "create dns name";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-no-create-dns-{}.com",
        *TIMESTAMP
    );
    let args = ["1", &qname, PLUGIN];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();
    let last = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_qname)) => {
                id_change == change.as_bytes() && id_qname == qname.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

#[tokio::test]
async fn test_changelog_no_create_node() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node";
    let change = "create plugin node";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-no-create-node-{}.com",
        *TIMESTAMP
    );
    let args = ["1", &qname, PLUGIN];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();
    let last = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_qname)) => {
                id_change == change.as_bytes() && id_qname == qname.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

#[tokio::test]
async fn test_changelog_no_create_report() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report";
    let change = "create report";
    let report = format!("changelog-no-create-report-{}", *TIMESTAMP);
    let args = ["1", &report, PLUGIN, "title", "0"];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();
    let last = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_report)) => {
                id_change == change.as_bytes() && id_report == report.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

// CREATE REPORT DATA

#[tokio::test]
async fn test_changelog_report_create_data_str() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "created data";
    let report = format!("changelog-report-create-data-str-{}", *TIMESTAMP);
    let data_key = format!("{REPORTS_KEY};{report};1");

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", &report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(
        &mut con,
        function,
        &[
            "1", &report, PLUGIN, "1", "string", "title", "plain", "content",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_report_create_data_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "created data";
    let report = format!("changelog-report-create-data-list-{}", *TIMESTAMP);
    let data_key = format!("{REPORTS_KEY};{report};1");

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", &report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &report,
            PLUGIN,
            "1",
            "list",
            "list_title",
            "item_title",
            "content",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_report_create_data_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "created data";
    let report = format!("changelog-report-create-data-hash-{}", *TIMESTAMP);
    let data_key = format!("{REPORTS_KEY};{report};1");

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", &report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &report,
            PLUGIN,
            "1",
            "hash",
            "title",
            "content_key",
            "content_val",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_report_create_data_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "created data";
    let report = format!("changelog-report-create-data-table-{}", *TIMESTAMP);
    let data_key = format!("{REPORTS_KEY};{report};1");

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", &report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &report,
            PLUGIN,
            "1",
            "table",
            "title",
            "3",
            "content_col1",
            "content_col2",
            "content_col3",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// UDPATE REPORT DATA

#[tokio::test]
async fn test_changelog_report_update_data_str() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "updated data";
    let report = format!("changelog-report-update-data-str-{}", *TIMESTAMP);
    let data_key = format!("{REPORTS_KEY};{report};1");
    let mut args = [
        "1", &report, PLUGIN, "1", "string", "title", "plain", "content",
    ];

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", &report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(&mut con, "netdox_create_report_data", &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_report_update_data_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "updated data";
    let report = "changelog-report-update-data-list";
    let data_key = format!("{REPORTS_KEY};{report};1");
    let mut args = [
        "1",
        report,
        PLUGIN,
        "1",
        "list",
        "list_title",
        "item_title",
        "content",
    ];

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_report_update_data_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "updated data";
    let report = format!("changelog-report-update-data-hash-{}", *TIMESTAMP);
    let data_key = format!("{REPORTS_KEY};{report};1");
    let mut args = [
        "1",
        &report,
        PLUGIN,
        "1",
        "hash",
        "title",
        "content_key",
        "content_val",
    ];

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", &report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[6] = "content_key_";
    args[7] = "content_val_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_report_update_data_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report_data";
    let change = "updated data";
    let report = "changelog-report-update-data-table";
    let data_key = format!("{REPORTS_KEY};{report};1");
    let mut args = [
        "1",
        report,
        PLUGIN,
        "1",
        "table",
        "title",
        "3",
        "content_col1",
        "content_col2",
        "content_col3",
    ];

    call_fn(
        &mut con,
        "netdox_create_report",
        &["1", report, PLUGIN, "title", "1"],
    )
    .await;

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_col1_";
    args[8] = "content_col2_";
    args[9] = "content_col3_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// CREATE DNS PLUGIN DATA

#[tokio::test]
async fn test_changelog_dns_create_data_str() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "created data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-create-data-str-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");

    call_fn(
        &mut con,
        function,
        &[
            "1", &qname, PLUGIN, "string", "1", "title", "plain", "content",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_create_data_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "created data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-create-data-list-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &qname,
            PLUGIN,
            "list",
            "1",
            "list_title",
            "item_title",
            "content",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_create_data_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "created data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-create-data-hash-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &qname,
            PLUGIN,
            "hash",
            "1",
            "title",
            "content_key",
            "content_val",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_create_data_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "created data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-create-data-table-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &qname,
            PLUGIN,
            "table",
            "1",
            "title",
            "3",
            "content_col1",
            "content_col2",
            "content_col3",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// UPDATE DNS PLUGIN DATA

#[tokio::test]
async fn test_changelog_dns_update_data_str() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-str-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let mut args = [
        "1", &qname, PLUGIN, "string", "1", "title", "plain", "content",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_update_data_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-list-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let mut args = [
        "1",
        &qname,
        PLUGIN,
        "list",
        "1",
        "list_title",
        "item_title",
        "content",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_update_data_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-hash-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let mut args = [
        "1",
        &qname,
        PLUGIN,
        "hash",
        "1",
        "title",
        "content_key",
        "content_val",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[6] = "content_key_";
    args[7] = "content_val_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_update_data_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-table-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let mut args = [
        "1",
        &qname,
        PLUGIN,
        "table",
        "1",
        "title",
        "3",
        "content_col1",
        "content_col2",
        "content_col3",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_col1_";
    args[8] = "content_col2_";
    args[9] = "content_col3_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// NO UPDATE DNS PLUGIN DATA

#[tokio::test]
async fn test_changelog_dns_no_update_data_str() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-no-update-data-str-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let args = [
        "1", &qname, PLUGIN, "string", "1", "title", "plain", "content",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

#[tokio::test]
async fn test_changelog_dns_no_update_data_list() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-no-update-data-list-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let args = [
        "1",
        &qname,
        PLUGIN,
        "list",
        "1",
        "list_title",
        "item_title",
        "content",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

#[tokio::test]
async fn test_changelog_dns_no_update_data_hash() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-no-update-data-hash-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let args = [
        "1",
        &qname,
        PLUGIN,
        "hash",
        "1",
        "title",
        "content_key",
        "content_val",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

#[tokio::test]
async fn test_changelog_dns_no_update_data_table() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-no-update-data-table-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let args = [
        "1",
        &qname,
        PLUGIN,
        "table",
        "1",
        "title",
        "3",
        "content_col1",
        "content_col2",
        "content_col3",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::Data(id_change), Value::Data(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}
