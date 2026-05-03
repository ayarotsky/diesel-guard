SET lock_timeout = '2s';
SET statement_timeout = '60s';
ALTER TABLE meeting_rooms ADD CONSTRAINT no_double_booking EXCLUDE USING gist (room_id WITH =, during WITH &&);
