ALTER TABLE meeting_rooms ADD CONSTRAINT no_double_booking EXCLUDE USING gist (room_id WITH =, during WITH &&);
