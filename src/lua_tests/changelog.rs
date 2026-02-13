use crate::{
    data::model::{CHANGELOG_KEY, DNS_KEY, METADATA_KEY, PDATA_KEY, REPORTS_KEY},
    tests_common::*,
};
use redis::{streams::StreamRangeReply, AsyncCommands, Value};

// CREATE OBJECTS

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
            (Value::BulkString(id_change), Value::BulkString(id_qname)) => {
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
    let name = format!("changelog-create-node-{}", *TIMESTAMP);
    let qname = format!("[{DEFAULT_NETWORK}]{name}.com",);

    call_fn(
        &mut con,
        function,
        &[
            "1",
            &qname,
            PLUGIN,
            &name,
            "false",
            &format!("{name}-link-id"),
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    dbg!(&changes);
    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_qname)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_report)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_qname)) => {
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
    let args = ["1", &qname, PLUGIN, &qname, "false", &qname];

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
            (Value::BulkString(id_change), Value::BulkString(id_qname)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_report)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");

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
            "1", &report, PLUGIN, "0", "string", "title", "plain", "content",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");

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
            "0",
            "list",
            "list_title",
            "name",
            "title",
            "value",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");

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
            "0",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");

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
            "0",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");
    let mut args = [
        "1", &report, PLUGIN, "0", "string", "title", "plain", "content",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");
    let mut args = [
        "1",
        report,
        PLUGIN,
        "0",
        "list",
        "list_title",
        "name",
        "title",
        "value",
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

    args[6] = "name_";
    args[7] = "title_";
    args[8] = "value_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");
    let mut args = [
        "1",
        &report,
        PLUGIN,
        "0",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{REPORTS_KEY};{report};0");
    let mut args = [
        "1",
        report,
        PLUGIN,
        "0",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};0");

    call_fn(
        &mut con,
        function,
        &[
            "1", &qname, PLUGIN, "string", "0", "title", "plain", "content",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            "name",
            "title",
            "value",
        ],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
        "name",
        "title",
        "value",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[6] = "name_";
    args[7] = "title_";
    args[8] = "value_";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_update_data_list_order() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-list-order-{}.com",
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
        "name1",
        "title1",
        "value1",
        "name2",
        "title2",
        "value2",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[6] = "name2";
    args[7] = "title2";
    args[8] = "value2";
    args[9] = "name1";
    args[10] = "title1";
    args[11] = "value1";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_update_data_hash_order() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-hash-order-{}.com",
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
        "content_key1",
        "content_val1",
        "content_key2",
        "content_val2",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[6] = "content_key2";
    args[7] = "content_val2";
    args[8] = "content_key1";
    args[9] = "content_val1";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_update_data_table_order() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "updated data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-update-data-table-order-{}.com",
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
        "content_col11",
        "content_col12",
        "content_col13",
        "content_col21",
        "content_col22",
        "content_col23",
    ];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[7] = "content_col21";
    args[8] = "content_col22";
    args[9] = "content_col23";
    args[10] = "content_col11";
    args[11] = "content_col12";
    args[12] = "content_col13";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
        "name",
        "title",
        "value",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
        "content_key1",
        "content_val1",
        "content_key2",
        "content_val2",
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

// NO CREATE EMPTY DNS DATA
// TODO add empty tests for all dtypes

#[tokio::test]
async fn test_changelog_dns_create_data_str_empty() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "created data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-create-data-str-empty-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");

    call_fn(
        &mut con,
        function,
        &["1", &qname, PLUGIN, "string", "1", "title", "plain", ""],
    )
    .await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_changelog_dns_create_data_list_empty() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_plugin_data";
    let change = "created data";
    let qname = format!(
        "[{DEFAULT_NETWORK}]changelog-plugin-no-create-data-list-empty-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{PDATA_KEY};{DNS_KEY};{qname};1");
    let args = ["1", &qname, PLUGIN, "list", "1", "list_title"];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// UPDATE METADATA

#[tokio::test]
async fn test_update_dns_meta() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let change = "updated metadata";
    let qname = format!("[{DEFAULT_NETWORK}]update-dns-meta-{}.com", *TIMESTAMP);
    let data_key = format!("{METADATA_KEY};{DNS_KEY};{qname}");

    call_fn(&mut con, function, &["1", &qname, PLUGIN, "key1", "val1-1"]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_update_dns_meta_change() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let change = "updated metadata";
    let qname = format!(
        "[{DEFAULT_NETWORK}]update-dns-meta-change-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{METADATA_KEY};{DNS_KEY};{qname}");
    let mut args = ["1", &qname, PLUGIN, "key", "val1"];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[4] = "val2";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_update_dns_meta_add() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let change = "updated metadata";
    let qname = format!("[{DEFAULT_NETWORK}]update-dns-meta-add-{}.com", *TIMESTAMP);
    let data_key = format!("{METADATA_KEY};{DNS_KEY};{qname}");
    let mut args = ["1", &qname, PLUGIN, "key1", "val"];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    args[3] = "key2";

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

#[tokio::test]
async fn test_update_dns_meta_empty() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let change = "updated metadata";
    let qname = format!(
        "[{DEFAULT_NETWORK}]update-dns-meta-empty-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{METADATA_KEY};{DNS_KEY};{qname}");

    call_fn(&mut con, function, &["1", &qname, PLUGIN]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, "-", "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(found_change)
}

// NO UPDATE METADATA

#[tokio::test]
async fn test_no_update_dns_meta() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let change = "updated metadata";
    let qname = format!("[{DEFAULT_NETWORK}]no-update-dns-meta-{}.com", *TIMESTAMP);
    let data_key = format!("{METADATA_KEY};{DNS_KEY};{qname}");
    let args = ["1", &qname, PLUGIN, "key", "val"];

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
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}

#[tokio::test]
async fn test_no_update_dns_meta_less() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns_metadata";
    let change = "updated metadata";
    let qname = format!(
        "[{DEFAULT_NETWORK}]no-update-dns-meta-less-{}.com",
        *TIMESTAMP
    );
    let data_key = format!("{METADATA_KEY};{DNS_KEY};{qname}");
    let args = ["1", &qname, PLUGIN, "key1", "val1", "key2", "val2"];

    call_fn(&mut con, function, &args).await;

    let changes: StreamRangeReply = con
        .xrevrange_count(CHANGELOG_KEY, "+", "-", 1)
        .await
        .unwrap();

    let last_change = format!("({}", changes.ids.last().unwrap().id);

    call_fn(&mut con, function, &["1", &qname, PLUGIN, "key1", "val1"]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last_change, "+").await.unwrap();

    let found_change = changes.ids.iter().any(|id| {
        match (id.map.get("change").unwrap(), id.map.get("value").unwrap()) {
            (Value::BulkString(id_change), Value::BulkString(id_data_key)) => {
                id_change == change.as_bytes() && id_data_key == data_key.as_bytes()
            }
            _ => false,
        }
    });

    assert!(!found_change)
}
