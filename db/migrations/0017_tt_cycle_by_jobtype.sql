-- per-jobtype TT cycle (DS discharge / LD load), stored alongside the headline (LD+DS).
ALTER TABLE raw_k_tt_cycle ADD COLUMN IF NOT EXISTS ds_samples INTEGER;
ALTER TABLE raw_k_tt_cycle ADD COLUMN IF NOT EXISTS ds_med_sec NUMERIC(10,2);
ALTER TABLE raw_k_tt_cycle ADD COLUMN IF NOT EXISTS ld_samples INTEGER;
ALTER TABLE raw_k_tt_cycle ADD COLUMN IF NOT EXISTS ld_med_sec NUMERIC(10,2);
