-- Accurate per-container ETW from the TOS ETW RPC, via the Azure tos_etw_gateway
-- (/v1/voyages/{vessel}/{voyage}/snapshot). Refilled each workpool tick. Joined to
-- live_workpool moves on (vessel, voyage, container) to replace the coarse DB ETW.
CREATE TABLE IF NOT EXISTS tos_etw_cntr (
  vessel          TEXT NOT NULL,
  voyage          TEXT NOT NULL,
  cntr_no         TEXT NOT NULL,
  dis_ld          TEXT,
  qc_etw_utc      TIMESTAMPTZ,
  vessel_etw_utc  TIMESTAMPTZ,
  fetched_at_utc  TIMESTAMPTZ,
  expires_at_utc  TIMESTAMPTZ,
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (vessel, voyage, cntr_no)
);
CREATE INDEX IF NOT EXISTS tos_etw_cntr_cntr_idx ON tos_etw_cntr (cntr_no);
