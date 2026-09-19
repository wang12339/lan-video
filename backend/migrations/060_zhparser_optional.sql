-- 060: 可选启用 zhparser 中文分词；未安装扩展时迁移只输出 NOTICE，零副作用。
--
-- 背景：内置 'simple' 词典不对中文分词，中文全文搜索此前只能走 pg_trgm
-- 子串匹配。若数据库装有 zhparser（https://github.com/amutu/zhparser），
-- 本迁移创建 `chinese` 文本搜索配置与表达式 GIN 索引；search_service.rs
-- 运行时探测到 parser = zhparser 的 chinese 配置后，中文查询改用原生分词。
--
-- 为什么不把 search_vector 切换到 chinese：
-- search_vector 由触发器用 'simple' 维护，且 video_repo.rs 的列表过滤
-- 硬编码 `search_vector @@ plainto_tsquery('simple', ...)`（不在本任务允许
-- 修改的文件内）。若按环境把 search_vector 切到 chinese，装了扩展的库上该
-- 路径会与 simple tsquery 静默失配。因此这里保持 search_vector 不变，中文
-- 查询在查询时使用表达式
--   to_tsvector('chinese', title || ' ' || COALESCE(description, ''))
-- 并为其建立表达式索引，保证有/无扩展都正确。
--
-- 幂等：对象均先做存在性检查或用 IF NOT EXISTS；CREATE EXTENSION / 配置
-- 创建失败只 NOTICE 不抛错（缺库、无超级用户权限的环境安全）。

DO $migration$
DECLARE
    ext_schema text;
BEGIN
    BEGIN
        CREATE EXTENSION IF NOT EXISTS zhparser;
    EXCEPTION WHEN OTHERS THEN
        RAISE NOTICE '060: zhparser 扩展不可用（%），跳过中文分词配置，保持 simple/pg_trgm 回退', SQLERRM;
        RETURN;
    END;

    -- 扩展可能安装在非 search_path 的 schema，解析实际 schema 以限定 parser 名
    SELECT n.nspname INTO ext_schema
    FROM pg_extension e
    JOIN pg_namespace n ON n.oid = e.extnamespace
    WHERE e.extname = 'zhparser';

    IF NOT EXISTS (
        SELECT 1
        FROM pg_ts_config c
        JOIN pg_ts_parser p ON p.oid = c.cfgparser
        WHERE c.cfgname = 'chinese' AND p.prsname = 'zhparser'
    ) THEN
        IF EXISTS (SELECT 1 FROM pg_ts_config WHERE cfgname = 'chinese') THEN
            -- 同名配置已存在但不是 zhparser（例如测试夹具用 COPY = simple
            -- 建的占位配置）。不能改建：其 token 类型属于 default parser，
            -- 添加 n,v,a,... 映射会报错。保持原样，运行时探测也会跳过它。
            RAISE NOTICE '060: 已存在非 zhparser 的 chinese 配置，跳过中文分词配置';
        ELSE
            EXECUTE format(
                'CREATE TEXT SEARCH CONFIGURATION chinese (PARSER = %I.zhparser)',
                ext_schema
            );
            RAISE NOTICE '060: 已创建 chinese 文本搜索配置 (zhparser)';
        END IF;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM pg_ts_config c
        JOIN pg_ts_parser p ON p.oid = c.cfgparser
        WHERE c.cfgname = 'chinese' AND p.prsname = 'zhparser'
    ) THEN
        -- ALTER MAPPING 对已有映射是覆盖、对缺失是新增，天然幂等；
        -- 映射集与 zhparser 官方 README 的常规配置一致。
        EXECUTE 'ALTER TEXT SEARCH CONFIGURATION chinese ALTER MAPPING FOR n,v,a,i,e,l WITH simple';

        -- 表达式索引：与 search_service.rs 中文查询的表达式逐字一致。
        EXECUTE $sql$
            CREATE INDEX IF NOT EXISTS idx_videos_search_zhparser
                ON videos USING gin (
                    to_tsvector('chinese', title || ' ' || COALESCE(description, ''))
                )
        $sql$;
        RAISE NOTICE '060: zhparser 中文表达式索引 idx_videos_search_zhparser 就绪';
    END IF;
END
$migration$;
