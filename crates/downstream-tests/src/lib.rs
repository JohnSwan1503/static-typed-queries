//! Uses the crates the way another project does: through a renamed dependency, on an older
//! edition, under strict lints.
#![deny(
    warnings,
    missing_docs,
    missing_debug_implementations,
    unreachable_pub,
    unsafe_code,
    rust_2018_idioms,
    unused_qualifications,
    unused_lifetimes,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    let_underscore_drop,
    redundant_lifetimes,
    unit_bindings,
    clippy::all,
    clippy::pedantic
)]

pub mod shadowed;

use std::marker::PhantomData;

use stq::prelude::*;

/// Notes each statement that uses `items`.
#[query(Sqlite, crate = stq, sql = "INSERT INTO notes (note) VALUES ({note})")]
#[derive(Debug)]
pub struct Note {
    /// The note.
    pub note: String,
}

/// The items for sale.
#[table(Sqlite, crate = stq, name = "items", before(Note))]
#[derive(Debug)]
pub struct Items;

/// An item.
#[derive(Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Item {
    /// Its id.
    pub id: i64,
    /// Its price.
    pub price: i64,
}

/// The items at or above a price.
#[query(
    Sqlite,
    crate = stq,
    row = Item,
    sql = "SELECT id, price FROM {Items} WHERE price >= {price} ORDER BY id"
)]
#[derive(Debug)]
pub struct Pricey {
    /// The lowest price.
    pub price: i64,
}

/// The ids of the items below a price.
#[sql]
pub const BELOW: &str = "SELECT id FROM {Items} WHERE price < {price}";

/// The items below a price.
#[query(Sqlite, crate = stq, sql = BELOW)]
#[derive(Debug)]
pub struct Cheap {
    /// The price.
    pub price: i64,
}

/// How many pricey items are also cheap.
#[query(Sqlite, crate = stq, sql = "SELECT count(*) FROM {pricey} WHERE id IN {cheap}")]
#[derive(Debug)]
pub struct Overlap {
    /// The pricey items.
    #[cte]
    pub pricey: Pricey,
    /// The cheap items.
    #[subquery]
    pub cheap: Cheap,
}

/// Counts the rows of a table or a query.
#[query(T::Dialect, crate = stq, sql = "SELECT count(*) FROM {T}")]
#[derive(Debug)]
pub struct CountOf<T: Sql>(PhantomData<T>);

/// The number of items.
#[statement(CountOf<Items>, crate = stq)]
#[derive(Debug)]
pub struct ItemCount;

/// Raises every price.
#[query(Sqlite, crate = stq, sql = "UPDATE {Items} SET price = price + {by}")]
#[derive(Debug)]
pub struct Raise {
    /// The amount.
    pub by: i64,
}

/// Raises the prices, then reads the pricey items.
#[transaction(Sqlite, crate = stq, steps(raise, savepoint(ItemCount), pricey))]
#[derive(Debug)]
pub struct Reprice {
    /// The raise.
    pub raise: Raise,
    /// The read.
    pub pricey: Pricey,
}

#[cfg(test)]
mod tests {
    use sqlx::{Connection, SqliteConnection};

    use super::*;

    #[test]
    fn renders() {
        assert_eq!(ItemCount::SQL, r#"SELECT count(*) FROM "items""#);
        assert_eq!(
            Overlap::SQL,
            r#"WITH "pricey" AS (SELECT id, price FROM "items" WHERE price >= $1 ORDER BY id) SELECT count(*) FROM "pricey" WHERE id IN (SELECT id FROM "items" WHERE price < $2)"#
        );
    }

    #[tokio::test]
    async fn runs() -> sqlx::Result<()> {
        let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
        sqlx::raw_sql(
            "CREATE TABLE items (id INTEGER PRIMARY KEY, price INTEGER);
             CREATE TABLE notes (note TEXT);
             INSERT INTO items VALUES (1, 5), (2, 20), (3, 30);",
        )
        .execute(&mut conn)
        .await?;

        let rows = Pricey::builder()
            .price(20)
            .with(Note {
                note: "read".to_owned(),
            })
            .run(&mut conn)
            .await?;
        assert_eq!(rows, [Item { id: 2, price: 20 }, Item { id: 3, price: 30 }]);

        let (raised, counted, pricey) = Reprice {
            raise: Raise { by: 10 },
            pricey: Pricey { price: 30 },
        }
        .with(Note {
            note: "reprice".to_owned(),
        })
        .run(&mut conn)
        .await?;
        assert_eq!(raised, 3);
        assert!(counted.is_ok());
        assert_eq!(
            pricey,
            [Item { id: 2, price: 30 }, Item { id: 3, price: 40 }]
        );

        let (notes,): (i64,) = sqlx::query_as("SELECT count(*) FROM notes")
            .fetch_one(&mut conn)
            .await?;
        assert_eq!(notes, 2);
        Ok(())
    }
}
