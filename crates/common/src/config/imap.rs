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

use std::time::Duration;

use utils::config::{Config, Rate};

#[derive(Default, Clone)]
pub struct ImapConfig {
    pub max_request_size: usize,
    pub max_auth_failures: u32,
    pub allow_plain_auth: bool,

    pub timeout_auth: Duration,
    pub timeout_unauth: Duration,
    pub timeout_idle: Duration,

    pub rate_requests: Option<Rate>,
    pub rate_concurrent: Option<u64>,
}

impl ImapConfig {
    pub fn parse(config: &mut Config) -> Self {
        ImapConfig {
            max_request_size: config
                .property_or_default("imap.request.max-size", "52428800")
                .unwrap_or(52428800),
            max_auth_failures: config
                .property_or_default("imap.auth.max-failures", "3")
                .unwrap_or(3),
            timeout_auth: config
                .property_or_default("imap.timeout.authenticated", "30m")
                .unwrap_or_else(|| Duration::from_secs(1800)),
            timeout_unauth: config
                .property_or_default("imap.timeout.anonymous", "1m")
                .unwrap_or_else(|| Duration::from_secs(60)),
            timeout_idle: config
                .property_or_default("imap.timeout.idle", "30m")
                .unwrap_or_else(|| Duration::from_secs(1800)),
            rate_requests: config
                .property_or_default::<Option<Rate>>("imap.rate-limit.requests", "2000/1m")
                .unwrap_or_default(),
            rate_concurrent: config
                .property::<Option<u64>>("imap.rate-limit.concurrent")
                .unwrap_or_default(),
            allow_plain_auth: config
                .property_or_default("imap.auth.allow-plain-text", "false")
                .unwrap_or(false),
        }
    }
}