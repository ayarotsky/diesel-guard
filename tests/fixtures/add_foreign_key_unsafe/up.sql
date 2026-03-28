-- Unsafe Adding foreign key without NOT VALID
SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE orders ADD CONSTRAINT fk_user_id
    FOREIGN KEY (user_id) REFERENCES users(id);