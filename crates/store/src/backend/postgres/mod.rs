/*
 * Copyright (c) 2023 Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Mail Server.
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

use deadpool_postgres::{Pool, PoolError};

pub mod blob;
pub mod lookup;
pub mod main;
pub mod read;
pub mod tls;
pub mod write;

pub struct PostgresStore {
    pub(crate) conn_pool: Pool,
}

impl From<PoolError> for crate::Error {
    fn from(err: PoolError) -> Self {
        Self::InternalError(format!("Connection pool error: {}", err))
    }
}

impl From<tokio_postgres::Error> for crate::Error {
    fn from(err: tokio_postgres::Error) -> Self {
        Self::InternalError(format!("PostgreSQL error: {}", err))
    }
}

#[inline(always)]
pub fn deserialize_bitmap(bytes: &[u8]) -> crate::Result<roaring::RoaringBitmap> {
    roaring::RoaringBitmap::deserialize_unchecked_from(bytes).map_err(|err| {
        crate::Error::InternalError(format!("Failed to deserialize bitmap: {}", err))
    })
}
