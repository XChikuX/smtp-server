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

use std::sync::Arc;

use utils::config::{cron::SimpleCron, utils::ParseValue, Config};

use crate::{
    backend::fs::FsStore,
    write::purge::{PurgeSchedule, PurgeStore},
    BlobStore, CompressionAlgo, FtsStore, LookupStore, QueryStore, Store, Stores,
};

#[cfg(feature = "s3")]
use crate::backend::s3::S3Store;

#[cfg(feature = "postgres")]
use crate::backend::postgres::PostgresStore;

#[cfg(feature = "mysql")]
use crate::backend::mysql::MysqlStore;

#[cfg(feature = "sqlite")]
use crate::backend::sqlite::SqliteStore;

#[cfg(feature = "foundation")]
use crate::backend::foundationdb::FdbStore;

#[cfg(feature = "rocks")]
use crate::backend::rocksdb::RocksDbStore;

#[cfg(feature = "elastic")]
use crate::backend::elastic::ElasticSearchStore;

#[cfg(feature = "redis")]
use crate::backend::redis::RedisStore;

impl Stores {
    pub async fn parse_all(config: &mut Config) -> Self {
        let mut stores = Self::parse(config).await;
        stores.parse_lookups(config).await;
        stores
    }

    pub async fn parse(config: &mut Config) -> Self {
        let mut stores = Self::default();
        stores.parse_stores(config).await;
        stores
    }

    pub async fn parse_stores(&mut self, config: &mut Config) {
        let is_reload = !self.stores.is_empty();

        for id in config
            .sub_keys("store", ".type")
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
        {
            let id = id.as_str();
            // Parse store
            #[cfg(feature = "test_mode")]
            {
                if config
                    .property_or_default::<bool>(("store", id, "disable"), "false")
                    .unwrap_or(false)
                {
                    tracing::debug!("Skipping disabled store {id:?}.");
                    continue;
                }
            }
            let protocol = if let Some(protocol) = config.value_require(("store", id, "type")) {
                protocol.to_ascii_lowercase()
            } else {
                continue;
            };
            let prefix = ("store", id);
            let store_id = id.to_string();
            let compression_algo = config
                .property_or_default::<CompressionAlgo>(("store", id, "compression"), "none")
                .unwrap_or(CompressionAlgo::None);

            match protocol.as_str() {
                #[cfg(feature = "rocks")]
                "rocksdb" => {
                    // Avoid opening the same store twice
                    if is_reload
                        && self
                            .stores
                            .values()
                            .any(|store| matches!(store, Store::RocksDb(_)))
                    {
                        continue;
                    }

                    if let Some(db) = RocksDbStore::open(config, prefix).await.map(Store::from) {
                        self.stores.insert(store_id.clone(), db.clone());
                        self.fts_stores.insert(store_id.clone(), db.clone().into());
                        self.blob_stores.insert(
                            store_id.clone(),
                            BlobStore::from(db.clone()).with_compression(compression_algo),
                        );
                        self.lookup_stores.insert(store_id, db.into());
                    }
                }
                #[cfg(feature = "foundation")]
                "foundationdb" => {
                    // Avoid opening the same store twice
                    if is_reload
                        && self
                            .stores
                            .values()
                            .any(|store| matches!(store, Store::FoundationDb(_)))
                    {
                        continue;
                    }

                    if let Some(db) = FdbStore::open(config, prefix).await.map(Store::from) {
                        self.stores.insert(store_id.clone(), db.clone());
                        self.fts_stores.insert(store_id.clone(), db.clone().into());
                        self.blob_stores.insert(
                            store_id.clone(),
                            BlobStore::from(db.clone()).with_compression(compression_algo),
                        );
                        self.lookup_stores.insert(store_id, db.into());
                    }
                }
                #[cfg(feature = "postgres")]
                "postgresql" => {
                    if let Some(db) = PostgresStore::open(config, prefix).await.map(Store::from) {
                        self.stores.insert(store_id.clone(), db.clone());
                        self.fts_stores.insert(store_id.clone(), db.clone().into());
                        self.blob_stores.insert(
                            store_id.clone(),
                            BlobStore::from(db.clone()).with_compression(compression_algo),
                        );
                        self.lookup_stores.insert(store_id.clone(), db.into());
                    }
                }
                #[cfg(feature = "mysql")]
                "mysql" => {
                    if let Some(db) = MysqlStore::open(config, prefix).await.map(Store::from) {
                        self.stores.insert(store_id.clone(), db.clone());
                        self.fts_stores.insert(store_id.clone(), db.clone().into());
                        self.blob_stores.insert(
                            store_id.clone(),
                            BlobStore::from(db.clone()).with_compression(compression_algo),
                        );
                        self.lookup_stores.insert(store_id.clone(), db.into());
                    }
                }
                #[cfg(feature = "sqlite")]
                "sqlite" => {
                    // Avoid opening the same store twice
                    if is_reload
                        && self
                            .stores
                            .values()
                            .any(|store| matches!(store, Store::SQLite(_)))
                    {
                        continue;
                    }

                    if let Some(db) = SqliteStore::open(config, prefix).map(Store::from) {
                        self.stores.insert(store_id.clone(), db.clone());
                        self.fts_stores.insert(store_id.clone(), db.clone().into());
                        self.blob_stores.insert(
                            store_id.clone(),
                            BlobStore::from(db.clone()).with_compression(compression_algo),
                        );
                        self.lookup_stores.insert(store_id.clone(), db.into());
                    }
                }
                "fs" => {
                    if let Some(db) = FsStore::open(config, prefix).await.map(BlobStore::from) {
                        self.blob_stores
                            .insert(store_id, db.with_compression(compression_algo));
                    }
                }
                #[cfg(feature = "s3")]
                "s3" => {
                    if let Some(db) = S3Store::open(config, prefix).await.map(BlobStore::from) {
                        self.blob_stores
                            .insert(store_id, db.with_compression(compression_algo));
                    }
                }
                #[cfg(feature = "elastic")]
                "elasticsearch" => {
                    if let Some(db) = ElasticSearchStore::open(config, prefix)
                        .await
                        .map(FtsStore::from)
                    {
                        self.fts_stores.insert(store_id, db);
                    }
                }
                #[cfg(feature = "redis")]
                "redis" => {
                    if let Some(db) = RedisStore::open(config, prefix)
                        .await
                        .map(LookupStore::from)
                    {
                        self.lookup_stores.insert(store_id, db);
                    }
                }
                unknown => {
                    tracing::debug!("Unknown directory type: {unknown:?}");
                }
            }
        }
    }

    pub async fn parse_lookups(&mut self, config: &mut Config) {
        // Parse memory stores
        self.parse_memory_stores(config);

        // Add SQL queries as lookup stores
        for (store_id, lookup_store) in self.stores.iter().filter_map(|(id, store)| {
            if store.is_sql() {
                Some((id.clone(), LookupStore::from(store.clone())))
            } else {
                None
            }
        }) {
            // Add queries as lookup stores
            for lookup_id in config.sub_keys(("store", store_id.as_str(), "query"), "") {
                if let Some(query) = config.value(("store", store_id.as_str(), "query", lookup_id))
                {
                    self.lookup_stores.insert(
                        format!("{store_id}/{lookup_id}"),
                        LookupStore::Query(Arc::new(QueryStore {
                            store: lookup_store.clone(),
                            query: query.to_string(),
                        })),
                    );
                }
            }

            // Run init queries on database
            for query in config
                .values(("store", store_id.as_str(), "init.execute"))
                .map(|(_, s)| s.to_string())
                .collect::<Vec<_>>()
            {
                if let Err(err) = lookup_store.query::<usize>(&query, Vec::new()).await {
                    config.new_build_error(
                        ("store", store_id.as_str()),
                        format!("Failed to initialize store: {err}"),
                    );
                }
            }
        }

        // Parse purge schedules
        if let Some(store) = config
            .value("storage.data")
            .and_then(|store_id| self.stores.get(store_id))
        {
            let store_id = config.value("storage.data").unwrap().to_string();
            self.purge_schedules.push(PurgeSchedule {
                cron: config
                    .property_or_default::<SimpleCron>(
                        ("store", store_id.as_str(), "purge.frequency"),
                        "0 3 *",
                    )
                    .unwrap_or_else(|| SimpleCron::parse_value("0 3 *").unwrap()),
                store_id,
                store: PurgeStore::Data(store.clone()),
            });

            if let Some(blob_store) = config
                .value("storage.blob")
                .and_then(|blob_store_id| self.blob_stores.get(blob_store_id))
            {
                let store_id = config.value("storage.blob").unwrap().to_string();
                self.purge_schedules.push(PurgeSchedule {
                    cron: config
                        .property_or_default::<SimpleCron>(
                            ("store", store_id.as_str(), "purge.frequency"),
                            "0 4 *",
                        )
                        .unwrap_or_else(|| SimpleCron::parse_value("0 4 *").unwrap()),
                    store_id,
                    store: PurgeStore::Blobs {
                        store: store.clone(),
                        blob_store: blob_store.clone(),
                    },
                });
            }
        }
        for (store_id, store) in &self.lookup_stores {
            if matches!(store, LookupStore::Store(_)) {
                self.purge_schedules.push(PurgeSchedule {
                    cron: config
                        .property_or_default::<SimpleCron>(
                            ("store", store_id.as_str(), "purge.frequency"),
                            "0 5 *",
                        )
                        .unwrap_or_else(|| SimpleCron::parse_value("0 5 *").unwrap()),
                    store_id: store_id.clone(),
                    store: PurgeStore::Lookup(store.clone()),
                });
            }
        }
    }
}

impl From<crate::Error> for String {
    fn from(err: crate::Error) -> Self {
        match err {
            crate::Error::InternalError(err) => err,
            crate::Error::AssertValueFailed => unimplemented!(),
        }
    }
}
