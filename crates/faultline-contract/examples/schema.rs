use faultline_contract::*;
use schemars::schema_for;
use serde_json::json;

fn main() {
    let schemas = json!({
        "health": schema_for!(IndexHealth),
        "search": schema_for!(SearchResponse),
        "impact": schema_for!(ImpactResponse),
        "references": schema_for!(ReferencesResponse),
        "graph": schema_for!(GraphResponse)
    });
    println!("{}", serde_json::to_string_pretty(&schemas).unwrap());
}
