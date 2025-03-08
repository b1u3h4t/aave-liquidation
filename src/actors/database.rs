use std::sync::Arc;

use crate::configs::DatabaseConfig;
use actix::prelude::*;
use sqlx::{PgPool, Row};

pub struct Database {
    pub pool: Arc<PgPool>,
}

impl Actor for Database {
    type Context = Context<Self>;
}

impl Database {
    pub async fn new(config: DatabaseConfig) -> Database {
        Database { pool: config.pool }
    }
}
