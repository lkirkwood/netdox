use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    data::model::{CHANGELOG_KEY, REPORTS_KEY},
    tests_common::*,
};
use lazy_static::lazy_static;
use redis::{streams::StreamRangeReply, AsyncCommands, Value};

lazy_static! {
    static ref TIMESTAMP: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
}

#[tokio::test]
async fn test_changelog_dns() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_dns";
    let change = "create dns name";
    let qname = format!("[{DEFAULT_NETWORK}]changelog-dns-{}.com", *TIMESTAMP);

    let changes: StreamRangeReply = con.xrange_count(CHANGELOG_KEY, "-", "+", 1).await.unwrap();
    let last = match changes.ids.last() {
        Some(change) => &change.id,
        None => "-",
    };

    call_fn(&mut con, function, &["1", &qname, PLUGIN]).await;

    let changes: StreamRangeReply = con.xrange(CHANGELOG_KEY, last, "+").await.unwrap();

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
async fn test_changelog_node() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_node";
    let change = "create plugin node";
    let qname = format!("[{DEFAULT_NETWORK}]changelog-node-{}.com", *TIMESTAMP);

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
async fn test_changelog_report() {
    let mut con = setup_db_con().await;
    let function = "netdox_create_report";
    let change = "create report";
    let report = format!("changelog-report-{}", *TIMESTAMP);

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
            "list",
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
        .xrevrange_count(CHANGELOG_KEY, "-", "+", 1)
        .await
        .unwrap();

    let last_change = match changes.ids.last() {
        Some(change) => format!("({}", change.id),
        None => "-".to_string(),
    };

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
        .xrevrange_count(CHANGELOG_KEY, "-", "+", 1)
        .await
        .unwrap();

    let last_change = match changes.ids.last() {
        Some(change) => format!("({}", change.id),
        None => "-".to_string(),
    };

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
        "list",
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
        .xrevrange_count(CHANGELOG_KEY, "-", "+", 1)
        .await
        .unwrap();

    let last_change = match changes.ids.last() {
        Some(change) => format!("({}", change.id),
        None => "-".to_string(),
    };

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
        .xrevrange_count(CHANGELOG_KEY, "-", "+", 1)
        .await
        .unwrap();

    let last_change = match changes.ids.last() {
        Some(change) => format!("({}", change.id),
        None => "-".to_string(),
    };

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
