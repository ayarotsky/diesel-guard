-- This migration demonstrates the parser limitation:
-- When UNIQUE USING INDEX causes a parse error, ALL statements are skipped

-- Safe operation (but causes parse failure)
ALTER TABLE users ADD CONSTRAINT users_email_key UNIQUE USING INDEX users_email_idx;

-- Unsafe operation that SHOULD be detected but WON'T be due to parser limitation
ALTER TABLE users DROP COLUMN old_field;
