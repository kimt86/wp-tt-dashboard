-- Authoritative soon-idle labels (ground truth): one row per completed TOS handover
-- (JOB_ORDER_HISTORY JOBSTATUS='C'), collected incrementally by the `handover` extractor via
-- etl_watermark (stream='handover_label') using the index IDX_JOBHIST_DATETIME on
-- (JOB_HIST_DATE||JOB_HIST_TIME). `comp_ts` is the authoritative moment the truck was freed —
-- the label to measure soon-idle accuracy against. For DS this is the ONLY ground truth (the
-- websocket has no RTG PLC). See research/soon-idle-tos (연구 2차, 다음단계 ③).
CREATE TABLE IF NOT EXISTS tos_handover_label (
  contno      TEXT    NOT NULL,            -- order natural key (= JOBORDER_PK_LIST: contno+point+seqno)
  point       BIGINT  NOT NULL,
  seqno       TEXT    NOT NULL,
  ytno        TEXT,                        -- the freed truck
  armgc       TEXT,                        -- yard crane (RTG) / quay crane that handed over
  jobtype     TEXT,                        -- DS / LD
  topos       TEXT,                        -- yard work-point
  dis_ts      TIMESTAMPTZ,                 -- truck arrived / discharged at block (YT_DIS_DT)
  actv_ts     TIMESTAMPTZ,                 -- crane/order activation (JOB_HIST_ACTV_DT)
  comp_ts     TIMESTAMPTZ NOT NULL,        -- completion event = truck freed (authoritative idle)
  captured_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (contno, point, seqno)
);
CREATE INDEX IF NOT EXISTS tos_handover_label_comp_idx ON tos_handover_label (comp_ts);
CREATE INDEX IF NOT EXISTS tos_handover_label_ytno_idx ON tos_handover_label (ytno, comp_ts);
