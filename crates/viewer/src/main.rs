use plotters::prelude::*;
use sqlx::sqlite::SqlitePoolOptions;

const USAGE: &str = "Usage: viewer <db_file> <output.png> [start] [count]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let db_file = args.get(1).ok_or(USAGE)?;
    let output_file = args.get(2).ok_or(USAGE)?;
    let start: i64 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
    let count: i64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(100);

    let pool = SqlitePoolOptions::new()
        .connect(&format!("sqlite:{}", db_file))
        .await?;

    let rows: Vec<(i64, f64)> =
        sqlx::query_as("SELECT id, power FROM sensor WHERE id >= ?1 ORDER BY id LIMIT ?2")
            .bind(start)
            .bind(count)
            .fetch_all(&pool)
            .await?;

    if rows.is_empty() {
        return Err("No data found".into());
    }

    let powers: Vec<f64> = rows.iter().map(|(_, p)| *p).collect();

    // fold() iterates through the collection, carrying an "accumulator" value.
    // - (0, 0.0) is the initial accumulator: (best_index_so_far, best_value_so_far)
    // - For each element, the closure receives:
    //   - (max_idx, max_val): the current accumulator (best we've seen)
    //   - (i, p): the current element's index (from enumerate) and value
    // - The closure returns the new accumulator:
    //   - If current value *p > max_val, return (i, *p) as the new best
    //   - Otherwise, keep the existing (max_idx, max_val)
    // - After all elements, fold returns the final accumulator
    let (idx_max_power, max_power) =
        powers
            .iter()
            .enumerate()
            .fold((0, 0.0), |(max_idx, max_val), (i, p)| {
                if *p > max_val {
                    // *p is larger return i and *p as tuple
                    (i, *p)
                } else {
                    // Return current values
                    (max_idx, max_val)
                }
            });
    println!("idx_max_power={idx_max_power}, max_power={max_power}");
    let max_power = max_power + 10.0;

    let root = BitMapBackend::new(output_file, (800, 400)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Power (W)", ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(40)
        .build_cartesian_2d(0f64..powers.len() as f64, 0f64..max_power)?;

    chart.configure_mesh().draw()?;
    chart.draw_series(LineSeries::new(
        powers.iter().enumerate().map(|(i, p)| (i as f64, *p)),
        &BLUE,
    ))?;

    root.present()?;
    println!("Saved to {} ({} points)", output_file, rows.len());

    Ok(())
}
