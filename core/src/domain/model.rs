use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SymbolDef {
    pub fqn: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: i64,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallerInfo {
    pub function: String,
    pub file: String,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalleeInfo {
    pub function: String,
    pub file: String,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub line: i64,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HotspotEntry {
    pub path: String,
    pub complexity: i64,
    pub dependents: i64,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Reference {
    pub file: String,
    pub line: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactResult {
    pub target: String,
    pub direct: Vec<String>,
    pub transitive: Vec<String>,
}

impl ImpactResult {
    pub fn total_affected(&self) -> usize {
        self.direct.len() + self.transitive.len()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExplainResult {
    pub fqn: String,
    pub file: String,
    pub line: i64,
    pub signature: String,
    pub callers: Vec<CallerInfo>,
    pub callees: Vec<CalleeInfo>,
    pub overrides: Vec<SymbolDef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceResult {
    pub source: String,
    pub target: String,
    pub path: Vec<CallerInfo>,
}

impl TraceResult {
    pub fn hops(&self) -> usize {
        self.path.len()
    }
}
