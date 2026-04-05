-- Add zone_id to meter_readings
ALTER TABLE meter_readings ADD COLUMN IF NOT EXISTS zone_id INTEGER;
CREATE INDEX IF NOT EXISTS idx_meter_readings_zone_id ON meter_readings(zone_id);
