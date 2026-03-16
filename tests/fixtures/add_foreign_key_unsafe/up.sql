-- Unsafe Adding foreign key without NOT VALID
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id);