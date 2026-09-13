-- 聊天室视频消息：
-- msg_type: 2=视频（content 为配文，可为空；video_url 指向 /media/chat/{file}.mp4|webm）
ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS video_url VARCHAR(255);
