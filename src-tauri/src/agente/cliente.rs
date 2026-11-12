//! PROVISIONAL (S16). El cliente del canal lo escribe S15; esto solo tiene su firma, la del
//! contrato, para que el servidor MCP compile antes de que exista la tubería. Al juntar se
//! queda el de S15.

/// Manda una petición del contrato a la app y devuelve su respuesta. Aquí siempre falla.
pub fn pedir(_peticion: &serde_json::Value) -> Result<serde_json::Value, String> {
    Err("This winshotx build has no agent channel yet, so it cannot reach the app.".into())
}
