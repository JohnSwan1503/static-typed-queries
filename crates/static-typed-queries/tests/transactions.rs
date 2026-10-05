use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    Postgres,
    sql = "SELECT id FROM {Orders} WHERE customer_id = {customer_id}"
)]
pub struct CustomerOrders {
    pub customer_id: i64,
}

#[query(T::Dialect, sql = "SELECT count(*) FROM {of}")]
pub struct CountOf<T: Sql> {
    #[cte]
    pub of: T,
}

#[query(Postgres, sql = "DELETE FROM {Orders} WHERE created_at < {before}")]
pub struct Archive {
    pub before: i64,
}

#[statement(count)]
pub struct OrderCount {
    pub count: CountOf<Orders>,
}

#[transaction(Postgres, steps(archive, count, order_count, archive))]
pub struct Nightly {
    pub archive: Archive,
    pub count: CountOf<CustomerOrders>,
    pub order_count: OrderCount,
}

#[query(
    Postgres,
    sql = "DELETE FROM {Orders} WHERE created_at < {before} RETURNING id"
)]
pub struct ArchiveIds {
    pub before: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM archive_ids")]
pub struct Archived;

#[transaction(
    Postgres,
    steps(archive_ids as cte, customer_orders as cte, Archived, order_count)
)]
pub struct Sweep {
    pub archive_ids: ArchiveIds,
    pub customer_orders: CustomerOrders,
    pub order_count: OrderCount,
}

#[test]
fn steps_render_in_order() {
    let steps: Vec<(&str, &str)> = Nightly::STEPS
        .iter()
        .map(|step| (step.name().as_str(), step.sql()))
        .collect();
    assert_eq!(
        steps,
        [
            ("archive", Archive::SQL),
            (
                "count_of",
                r#"WITH "customer_orders" AS (SELECT id FROM "orders" WHERE customer_id = $1) SELECT count(*) FROM "customer_orders""#
            ),
            ("order_count", OrderCount::SQL),
            ("archive", Archive::SQL),
        ]
    );
}

#[test]
fn steps_bind_from_the_transaction_fields() {
    let paths: Vec<Vec<Vec<u16>>> = Nightly::STEPS
        .iter()
        .map(|step| {
            step.binds()
                .iter()
                .map(|bind| bind.path().steps().to_vec())
                .collect()
        })
        .collect();
    assert_eq!(
        paths,
        [vec![vec![0]], vec![vec![1, 0]], vec![], vec![vec![0]]]
    );
}

mod lite {
    use static_typed_queries::prelude::*;

    #[query(Sqlite, sql = "UPDATE counters SET n = n + 1 WHERE name = {name}")]
    pub struct Bump {
        pub name: String,
    }

    #[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ({note})")]
    pub struct Note {
        pub note: String,
    }

    #[table(Sqlite, name = "items", before(Bump), after(Note))]
    pub struct Items;

    #[query(
        Sqlite,
        sql = "UPDATE {Items} SET price = price * 2 WHERE price > {price}"
    )]
    pub struct Double {
        pub price: i64,
    }

    #[query(Sqlite, sql = "INSERT INTO {Items} (id, price) VALUES ({id}, {price})")]
    pub struct AddItem {
        pub id: i64,
        pub price: i64,
    }

    #[derive(sqlx::FromRow, Debug, PartialEq)]
    pub struct Item {
        pub id: i64,
        pub price: i64,
    }

    #[query(Sqlite, row = Item, sql = "SELECT id, price FROM {Items} ORDER BY id")]
    pub struct Stock;

    #[transaction(Sqlite, steps(double, add_item, Stock))]
    pub struct Restock {
        pub double: Double,
        pub add_item: AddItem,
    }

    #[transaction(Sqlite, steps(double, savepoint(add_item, double), Stock))]
    pub struct Careful {
        pub double: Double,
        pub add_item: AddItem,
    }

    #[transaction(Sqlite, steps(savepoint(double, savepoint(add_item)), Stock))]
    pub struct Nested {
        pub double: Double,
        pub add_item: AddItem,
    }

    #[query(Sqlite, row = Item, sql = "SELECT id, price FROM {Items} WHERE id = {id}")]
    pub struct ItemById {
        pub id: i64,
    }

    #[query(
        Sqlite,
        row = Item,
        sql = "SELECT id, price FROM {Items} WHERE price < {below} ORDER BY price LIMIT 1"
    )]
    pub struct Cheapest {
        pub below: i64,
    }

    #[transaction(Sqlite, steps(double, item as one, cheapest as optional))]
    pub struct Lookup {
        pub double: Double,
        pub item: ItemById,
        pub cheapest: Cheapest,
    }
}

use lite::{
    AddItem, Bump, Careful, Cheapest, Double, Item, ItemById, Lookup, Nested, Note, Restock,
};
use sqlx::{Connection, SqliteConnection};

async fn connect() -> sqlx::Result<SqliteConnection> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, price INTEGER);
         CREATE TABLE counters (name TEXT PRIMARY KEY, n INTEGER);
         CREATE TABLE audit (note TEXT);
         INSERT INTO items VALUES (1, 5), (2, 20);
         INSERT INTO counters VALUES ('runs', 0);",
    )
    .execute(&mut conn)
    .await?;
    Ok(conn)
}

async fn hooks(conn: &mut SqliteConnection) -> sqlx::Result<(i64, Vec<String>)> {
    let (n,): (i64,) = sqlx::query_as("SELECT n FROM counters")
        .fetch_one(&mut *conn)
        .await?;
    let notes: Vec<(String,)> = sqlx::query_as("SELECT note FROM audit")
        .fetch_all(&mut *conn)
        .await?;
    Ok((n, notes.into_iter().map(|(note,)| note).collect()))
}

fn bump() -> Bump {
    Bump {
        name: "runs".to_owned(),
    }
}

fn note(note: &str) -> Note {
    Note {
        note: note.to_owned(),
    }
}

async fn restock(
    conn: &mut SqliteConnection,
    id: i64,
    note: &str,
) -> sqlx::Result<(u64, u64, Vec<Item>)> {
    Restock {
        double: Double { price: 10 },
        add_item: AddItem { id, price: 30 },
    }
    .with(bump())
    .with(self::note(note))
    .run(conn)
    .await
}

#[tokio::test]
async fn run_returns_the_output_of_each_step() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let (doubled, added, stock) = restock(&mut conn, 3, "restock").await?;
    assert_eq!((doubled, added), (1, 1));
    assert_eq!(
        stock,
        [
            Item { id: 1, price: 5 },
            Item { id: 2, price: 40 },
            Item { id: 3, price: 30 },
        ]
    );
    assert_eq!(hooks(&mut conn).await?, (1, vec!["restock".to_owned()]));
    Ok(())
}

#[tokio::test]
async fn a_failing_step_rolls_back_every_step() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let error = restock(&mut conn, 1, "again").await.unwrap_err();
    assert!(error.to_string().contains("UNIQUE"), "{error}");
    let prices: Vec<(i64, i64)> = sqlx::query_as("SELECT id, price FROM items ORDER BY id")
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(prices, [(1, 5), (2, 20)]);
    assert_eq!(hooks(&mut conn).await?, (0, vec![]));
    Ok(())
}

#[test]
fn runs_are_send() {
    fn send<T: Send>(_: T) {}
    let _ = |conn: &mut SqliteConnection| send(restock(conn, 3, "send"));
}

async fn careful(
    conn: &mut SqliteConnection,
    id: i64,
) -> sqlx::Result<(u64, sqlx::Result<(u64, u64)>, Vec<Item>)> {
    Careful {
        double: Double { price: 10 },
        add_item: AddItem { id, price: 30 },
    }
    .with(bump())
    .with(note("careful"))
    .run(conn)
    .await
}

#[tokio::test]
async fn a_savepoint_commits_with_the_transaction() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let (doubled, group, stock) = careful(&mut conn, 3).await?;
    assert_eq!(doubled, 1);
    assert_eq!(group?, (1, 2));
    assert_eq!(
        stock,
        [
            Item { id: 1, price: 5 },
            Item { id: 2, price: 80 },
            Item { id: 3, price: 60 },
        ]
    );
    Ok(())
}

#[tokio::test]
async fn a_failing_savepoint_rolls_back_only_its_steps() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let (doubled, group, stock) = careful(&mut conn, 1).await?;
    assert_eq!(doubled, 1);
    let error = group.unwrap_err();
    assert!(error.to_string().contains("UNIQUE"), "{error}");
    assert_eq!(stock, [Item { id: 1, price: 5 }, Item { id: 2, price: 40 }]);
    assert_eq!(hooks(&mut conn).await?, (1, vec!["careful".to_owned()]));
    Ok(())
}

#[tokio::test]
async fn savepoints_nest() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let (outer, stock) = Nested {
        double: Double { price: 10 },
        add_item: AddItem { id: 1, price: 1 },
    }
    .with(bump())
    .with(note("nested"))
    .run(&mut conn)
    .await?;
    let (doubled, inner) = outer?;
    assert_eq!(doubled, 1);
    assert!(inner.is_err());
    assert_eq!(stock, [Item { id: 1, price: 5 }, Item { id: 2, price: 40 }]);
    Ok(())
}

async fn lookup(
    conn: &mut SqliteConnection,
    id: i64,
    below: i64,
) -> sqlx::Result<(u64, Item, Option<Item>)> {
    Lookup {
        double: Double { price: 10 },
        item: ItemById { id },
        cheapest: Cheapest { below },
    }
    .with(bump())
    .with(note("lookup"))
    .run(conn)
    .await
}

#[tokio::test]
async fn steps_can_read_one_row_or_an_optional_row() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let (_, item, cheapest) = lookup(&mut conn, 2, 10).await?;
    assert_eq!(item, Item { id: 2, price: 40 });
    assert_eq!(cheapest, Some(Item { id: 1, price: 5 }));

    let (_, _, none) = lookup(&mut connect().await?, 1, 1).await?;
    assert_eq!(none, None);

    let error = lookup(&mut conn, 9, 10).await.unwrap_err();
    assert!(matches!(error, sqlx::Error::RowNotFound), "{error}");
    Ok(())
}

#[test]
fn cte_steps_attach_to_the_next_step() {
    assert_eq!(Sweep::STEPS.len(), 2);
    assert_eq!(Sweep::STEPS[0].name().as_str(), "archived");
    assert_eq!(
        Sweep::STEPS[0].sql(),
        r#"WITH "archive_ids" AS (DELETE FROM "orders" WHERE created_at < $1 RETURNING id), "customer_orders" AS (SELECT id FROM "orders" WHERE customer_id = $2) SELECT count(*) FROM archive_ids"#
    );
    let paths: Vec<Vec<u16>> = Sweep::STEPS[0]
        .binds()
        .iter()
        .map(|bind| bind.path().steps().to_vec())
        .collect();
    assert_eq!(paths, [vec![0], vec![1]]);
    assert_eq!(Sweep::STEPS[1].sql(), OrderCount::SQL);
}

async fn _sweep(conn: &mut sqlx::PgConnection) -> sqlx::Result<(u64, u64)> {
    Sweep {
        archive_ids: ArchiveIds { before: 5 },
        customer_orders: CustomerOrders { customer_id: 7 },
        order_count: OrderCount {
            count: CountOf { of: Orders },
        },
    }
    .run(conn)
    .await
}
