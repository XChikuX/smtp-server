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

use common::DeliveryEvent;
use tokio::sync::mpsc;

use crate::{JmapInstance, JMAP};

pub fn spawn_delivery_manager(core: JmapInstance, mut delivery_rx: mpsc::Receiver<DeliveryEvent>) {
    tokio::spawn(async move {
        while let Some(event) = delivery_rx.recv().await {
            match event {
                DeliveryEvent::Ingest { message, result_tx } => {
                    result_tx
                        .send(JMAP::from(core.clone()).deliver_message(message).await)
                        .ok();
                }
                DeliveryEvent::Stop => break,
            }
        }
    });
}
