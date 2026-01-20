-- Safe: GENERATED STORED in CREATE TABLE is safe (table is empty)
CREATE TABLE products (
    id SERIAL PRIMARY KEY,
    price INTEGER NOT NULL,
    quantity INTEGER NOT NULL,
    total_price INTEGER GENERATED ALWAYS AS (price * quantity) STORED
);
