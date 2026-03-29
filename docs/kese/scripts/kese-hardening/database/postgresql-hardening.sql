-- KESE PostgreSQL hardening draft for RecordRoute
-- 상태: 초안 / 미적용
-- 목적: 운영 적용 전 검토용 baseline 제안
--
-- 사용 방법
-- 1. <app_db>, <app_role>, <admin_role>, <readonly_role>를 실제 값으로 치환합니다.
-- 2. staging 환경에서 먼저 검증합니다.
-- 3. pg_hba.conf, postgresql.conf, SSL/TLS 설정은 별도 운영 절차로 적용합니다.

-- 1. PUBLIC 기본 권한 축소
REVOKE ALL ON DATABASE <app_db> FROM PUBLIC;
REVOKE ALL ON SCHEMA public FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;

-- 2. 애플리케이션 role 최소 권한화
ALTER ROLE <app_role> NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;
ALTER ROLE <readonly_role> NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION;

-- 3. 연결 및 schema 사용 권한 재부여
GRANT CONNECT, TEMP ON DATABASE <app_db> TO <app_role>;
GRANT CONNECT ON DATABASE <app_db> TO <readonly_role>;
GRANT USAGE ON SCHEMA public TO <app_role>;
GRANT USAGE ON SCHEMA public TO <readonly_role>;

-- 4. 기존 객체 권한 정리
REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC;
REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA public FROM PUBLIC;

GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO <app_role>;
GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA public TO <app_role>;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO <app_role>;

GRANT SELECT ON ALL TABLES IN SCHEMA public TO <readonly_role>;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO <readonly_role>;

-- 5. 향후 생성 객체의 기본 권한 정리
ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  REVOKE ALL ON TABLES FROM PUBLIC;
ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  REVOKE ALL ON SEQUENCES FROM PUBLIC;
ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  REVOKE ALL ON FUNCTIONS FROM PUBLIC;

ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO <app_role>;
ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO <app_role>;
ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  GRANT EXECUTE ON FUNCTIONS TO <app_role>;

ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  GRANT SELECT ON TABLES TO <readonly_role>;
ALTER DEFAULT PRIVILEGES FOR ROLE <admin_role> IN SCHEMA public
  GRANT USAGE, SELECT ON SEQUENCES TO <readonly_role>;

-- 6. 세션 기본값 강화 예시
ALTER ROLE <app_role> SET search_path = public;
ALTER ROLE <app_role> SET statement_timeout = '30s';
ALTER ROLE <app_role> SET idle_in_transaction_session_timeout = '15s';
ALTER ROLE <readonly_role> SET search_path = public;
ALTER ROLE <readonly_role> SET statement_timeout = '15s';

-- 7. 수동 확인 필요 항목
-- - password_encryption = 'scram-sha-256'
-- - ssl = on
-- - pg_hba.conf 에서 trust/peer/no-password 접근 제거
-- - log_connections, log_disconnections, log_statement 정책 검토
-- - backup 계정과 app 계정 분리
