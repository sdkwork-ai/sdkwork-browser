-- sdkwork:migration
-- id: 0001_organization_id_not_null
-- engine: postgres
-- module: sdkwork-browser
-- purpose: Enforce organization_id NOT NULL DEFAULT on all tables in the
--   consolidated baseline. NULL rows (pre-standard data anomalies) are
--   backfilled with the platform sentinel before NOT NULL is set, and
--   NOT NULL columns without an explicit default receive the sentinel
--   default, keeping existing deployments consistent with fresh baseline
--   installs.
-- reversible: false
-- rollback: forward-fix (sentinel backfill is the canonical fix; NULL
--   organization rows are data anomalies)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE browser_session ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE browser_session SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE browser_session ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE browser_session ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE browser_bookmark ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE browser_bookmark SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE browser_bookmark ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE browser_bookmark ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE browser_history ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE browser_history SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE browser_history ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE browser_history ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE browser_tab ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE browser_tab SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE browser_tab ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE browser_tab ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE browser_download ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE browser_download SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE browser_download ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE browser_download ALTER COLUMN organization_id SET NOT NULL;

COMMIT;
