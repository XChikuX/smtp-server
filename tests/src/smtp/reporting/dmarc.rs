/*
 * Copyright (c) 2023 Stalwart Labs Ltd.
 *
 * This file is part of Stalwart Mail Server.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::{
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use common::config::smtp::report::AggregateFrequency;
use mail_auth::{
    common::parse::TxtRecordParser,
    dmarc::Dmarc,
    report::{ActionDisposition, Disposition, DmarcResult, Record, Report},
};
use store::write::QueueClass;

use smtp::reporting::DmarcEvent;

use crate::smtp::{
    inbound::{sign::SIGNATURES, TestMessage},
    outbound::TestServer,
    session::VerifyResponse,
};

const CONFIG: &str = r#"
[session.rcpt]
relay = true

[report]
submitter = "'mx.example.org'"

[report.dmarc.aggregate]
from-name = "'DMARC Report'"
from-address = "'reports@example.org'"
org-name = "'Foobar, Inc.'"
contact-info = "'https://foobar.org/contact'"
send = "daily"
max-size = 4096
sign = "['rsa']"

"#;

#[tokio::test]
async fn report_dmarc() {
    /*tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish(),
    )
    .unwrap();*/

    // Create scheduler
    let mut local = TestServer::new(
        "smtp_report_dmarc_test",
        CONFIG.to_string() + SIGNATURES,
        true,
    )
    .await;

    // Authorize external report for foobar.org
    let core = local.build_smtp();
    core.core.smtp.resolvers.dns.txt_add(
        "foobar.org._report._dmarc.foobar.net",
        Dmarc::parse(b"v=DMARC1;").unwrap(),
        Instant::now() + Duration::from_secs(10),
    );
    let qr = &mut local.qr;

    // Schedule two events with a same policy and another one with a different policy
    let dmarc_record = Arc::new(
        Dmarc::parse(
            b"v=DMARC1; p=quarantine; rua=mailto:reports@foobar.net,mailto:reports@example.net",
        )
        .unwrap(),
    );
    assert_eq!(dmarc_record.rua().len(), 2);
    for _ in 0..2 {
        core.schedule_dmarc(Box::new(DmarcEvent {
            domain: "foobar.org".to_string(),
            report_record: Record::new()
                .with_source_ip("192.168.1.2".parse().unwrap())
                .with_action_disposition(ActionDisposition::Pass)
                .with_dmarc_dkim_result(DmarcResult::Pass)
                .with_dmarc_spf_result(DmarcResult::Fail)
                .with_envelope_from("hello@example.org")
                .with_envelope_to("other@example.org")
                .with_header_from("bye@example.org"),
            dmarc_record: dmarc_record.clone(),
            interval: AggregateFrequency::Weekly,
        }))
        .await;
    }
    core.schedule_dmarc(Box::new(DmarcEvent {
        domain: "foobar.org".to_string(),
        report_record: Record::new()
            .with_source_ip("a:b:c::e:f".parse().unwrap())
            .with_action_disposition(ActionDisposition::Reject)
            .with_dmarc_dkim_result(DmarcResult::Fail)
            .with_dmarc_spf_result(DmarcResult::Pass),
        dmarc_record: dmarc_record.clone(),
        interval: AggregateFrequency::Weekly,
    }))
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let reports = qr.read_report_events().await;
    assert_eq!(reports.len(), 1);
    match reports.into_iter().next().unwrap() {
        QueueClass::DmarcReportHeader(event) => {
            core.send_dmarc_aggregate_report(event).await;
        }
        _ => unreachable!(),
    }

    // Expect report
    let message = qr.expect_message().await;
    qr.assert_no_events();
    assert_eq!(message.recipients.len(), 1);
    assert_eq!(
        message.recipients.last().unwrap().address,
        "reports@foobar.net"
    );
    assert_eq!(message.return_path, "reports@example.org");
    message
        .read_lines(qr)
        .await
        .assert_contains("DKIM-Signature: v=1; a=rsa-sha256; s=rsa; d=example.com;")
        .assert_contains("To: <reports@foobar.net>")
        .assert_contains("Report Domain: foobar.org")
        .assert_contains("Submitter: mx.example.org");

    // Verify generated report
    let report = Report::parse_rfc5322(message.read_message(qr).await.as_bytes()).unwrap();
    assert_eq!(report.domain(), "foobar.org");
    assert_eq!(report.email(), "reports@example.org");
    assert_eq!(report.org_name(), "Foobar, Inc.");
    assert_eq!(
        report.extra_contact_info().unwrap(),
        "https://foobar.org/contact"
    );
    assert_eq!(report.p(), Disposition::Quarantine);
    assert_eq!(report.records().len(), 2);
    for record in report.records() {
        let source_ip = record.source_ip().unwrap();
        if source_ip == "192.168.1.2".parse::<IpAddr>().unwrap() {
            assert_eq!(record.count(), 2);
            assert_eq!(record.action_disposition(), ActionDisposition::Pass);
            assert_eq!(record.envelope_from(), "hello@example.org");
            assert_eq!(record.header_from(), "bye@example.org");
            assert_eq!(record.envelope_to().unwrap(), "other@example.org");
        } else if source_ip == "a:b:c::e:f".parse::<IpAddr>().unwrap() {
            assert_eq!(record.count(), 1);
            assert_eq!(record.action_disposition(), ActionDisposition::Reject);
        } else {
            panic!("unexpected ip {source_ip}");
        }
    }
    qr.assert_report_is_empty().await;
}
