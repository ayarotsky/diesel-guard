-- Safe: DELETE with WHERE clause
DELETE FROM users WHERE deactivated_at < '2020-01-01';
