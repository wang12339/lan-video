-- 弹幕表：与视频、用户关联，记录出现时间/颜色/字号
CREATE TABLE danmaku (
    id          BIGSERIAL PRIMARY KEY,
    video_id   BIGINT NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    user_id    BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    text       TEXT NOT NULL,
    "time"     DOUBLE PRECISION NOT NULL DEFAULT 0,
    color      VARCHAR(16),
    font_size  INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_danmaku_video_id ON danmaku(video_id);
