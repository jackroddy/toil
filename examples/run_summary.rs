//! A pipeline's run summary: what each step ran, what it cost, and how it went.
//!
//! The marker is absorbed into the first column, so the step names start at the
//! left margin. The commands under a step are marked rather than named, and the
//! mark is right-aligned against the names above it. `argv` is last and ragged,
//! since its width belongs to the row and not to the table.

use toil::{Align, Cell, Column, Table};

fn main() {
    let mut columns: Vec<Column> = ["step", "cmd", "job"]
        .into_iter()
        .map(Column::new)
        .collect();
    columns.push(Column::new("wall(s)").fixed(2));
    columns.push(Column::new("user(s)").fixed(2));
    columns.push(Column::new("cpu(%)"));
    columns.push(Column::new("max_rss"));
    columns.push(Column::new("exit"));
    columns.push(Column::new("status"));
    columns.push(Column::new("argv").ragged());

    let mut table = Table::new(columns);

    table.row([
        Cell::from("[1](fetch)"),
        Cell::from("curl"),
        Cell::missing(),
        Cell::from(12.41),
        Cell::from(0.88),
        Cell::from("7%"),
        Cell::from("14.2MiB"),
        Cell::from(0),
        Cell::from("ok"),
        Cell::from("curl -sSL https://example.invalid/pfam.gz -o pfam.gz"),
    ]);

    table.row([
        Cell::from("[2](search)"),
        Cell::missing(),
        Cell::missing(),
        Cell::from(94.02),
        Cell::from(361.75),
        Cell::from("384%"),
        Cell::from("2.41GiB"),
        Cell::missing(),
        Cell::missing(),
        Cell::missing(),
    ]);

    let shards: [(u32, f64, f64, &str, i32, &str); 4] = [
        (0, 91.30, 90.12, "612MiB", 0, "ok"),
        (1, 94.02, 92.88, "2.41GiB", 0, "ok"),
        (2, 30.11, 29.40, "588MiB", 137, "fail"),
        (3, 0.00, 0.00, "-", 0, "skip"),
    ];

    for (shard, wall, user, rss, exit, status) in shards {
        table.row([
            Cell::from("||").align(Align::Right),
            Cell::from("nail"),
            Cell::from(shard),
            Cell::from(wall),
            Cell::from(user),
            Cell::from(format!("{:.0}%", user / wall.max(0.01) * 100.0)),
            Cell::from(rss),
            Cell::from(exit),
            Cell::from(status),
            Cell::from(format!("nail search --shard {shard} pfam.hmm targets.fa")),
        ]);
    }

    table.row([
        Cell::from("[3](report)"),
        Cell::from("tabulate"),
        Cell::missing(),
        Cell::from(0.44),
        Cell::from(0.41),
        Cell::from("93%"),
        Cell::from("31.0MiB"),
        Cell::from(0),
        Cell::from("ok"),
        Cell::from("tabulate results/ -o summary.tbl"),
    ]);

    print!("{}", table.render());
}
