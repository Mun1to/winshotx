//! El canal para agentes de IA. PROVISIONAL (S16): aquí solo está lo que necesita el
//! servidor MCP para compilar. El canal de verdad (la tubería, la orden `agent` y el
//! cliente) lo trae S15, y al juntar se queda su `mod.rs` y su `cliente.rs`.

pub mod cliente;
pub mod mcp;
