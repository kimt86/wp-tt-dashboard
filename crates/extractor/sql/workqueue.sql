-- Live per-QC work-queue plan (JOB_QUEUE_SCHEDULE). Each row is one (crane, vessel,
-- queue) chunk the QC works in JOB_QUE_SEQ order; TOTALQTY/COMPQTY give progress.
-- Bounded to currently-relevant queues: not deleted, not finished, touched within ~1
-- day. Small result (tens of rows). No date token: this is the live state right now.
-- JOB_QUE_ACTIVEYN is unreliable (NULL in practice) so it is NOT used as a filter.
SELECT
  s.JOB_QUE_CRANENO   AS qc,
  s.JOB_QUE_VESSEL    AS vessel,
  s.JOB_QUE_VOYAGE    AS voyage,
  s.JOB_QUE_QUEUENAME AS queuename,
  s.JOB_QUE_DISLOAD   AS disload,
  s.JOB_QUE_SEQ       AS seq,
  s.JOB_QUE_TOTALQTY  AS total_qty,
  s.JOB_QUE_COMPQTY   AS comp_qty,
  s.JOB_QUE_PLANQTY   AS plan_qty
FROM TOSADM.JOB_QUEUE_SCHEDULE s
WHERE NVL(s.DELT_FLG, 'N') <> 'Y'
  AND s.JOB_QUE_CRANENO IS NOT NULL
  AND NVL(s.JOB_QUE_TOTALQTY, 0) > NVL(s.JOB_QUE_COMPQTY, 0)
  AND s.UPD_DT >= TRUNC(SYSDATE) - 1
ORDER BY s.JOB_QUE_CRANENO, s.JOB_QUE_SEQ
