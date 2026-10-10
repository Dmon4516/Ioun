# Migraciones

Formato: `NNNN_descripcion.sql` (orden lexicográfico).

Se ejecutan con `sqlx::migrate!` al arrancar. **Nunca** editar una migración ya aplicada; crear una nueva.
