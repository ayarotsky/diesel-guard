-- Safe: UPDATE with WHERE clause
UPDATE users SET active = false WHERE last_login < '2020-01-01';
