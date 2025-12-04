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

    let rows: Vec<(i64, f64)> = sqlx::query_as(
        "SELECT id, power FROM sensor WHERE id >= ?1 ORDER BY id LIMIT ?2",
    )
    .bind(start)
    .bind(count)
    .fetch_all(&pool)
    .await?;

    if rows.is_empty() {
        return Err("No data found".into());
    }

    let powers: Vec<(f64, f64)> = rows
        .iter()
        .enumerate()
        .map(|(i, (_, p))| (i as f64, *p))
        .collect();

    let max_power = powers.iter().map(|(_, p)| *p).fold(0.0, f64::max) + 10.0;

    let root = BitMapBackend::new(output_file, (800, 400)).into_drawing_area();
    root.fill(&WHITE)?;

    let mut chart = ChartBuilder::on(&root)
        .caption("Power (W)", ("sans-serif", 20))
        .margin(10)
        .x_label_area_size(30)
        .y_label_area_size(40)
        .build_cartesian_2d(0f64..powers.len() as f64, 0f64..max_power)?;

    chart.configure_mesh().draw()?;
    chart.draw_series(LineSeries::new(powers, &BLUE))?;

    root.present()?;
    println!("Saved to {} ({} points)", output_file, rows.len());

    Ok(())
}
