// =================================================================
// UPLOAD — file/folder upload with pre-check dedup + progress + retry
// =================================================================
import { Dom } from './dom.js';
import { Api } from './api.js';
import { loadHomeFeed } from './home.js';

export async function startUpload(files) {
    if (!files || !files.length) return;

    const fileList = Array.from(files);

    Dom.uploadOverlay.classList.remove('hidden');
    Dom.uploadProgressFill.style.width = '0%';
    Dom.uploadProgressText.textContent = '0%';

    // Phase 1: pre-check dedup
    Dom.uploadStatus.textContent = `正在检查 ${fileList.length} 个文件...`;
    let toUpload, skipCount = 0;
    try {
        const metas = fileList.map(f => ({ name: f.name, size: f.size }));
        const existingIndices = await Api.checkFiles(metas);
        skipCount = existingIndices.length;
        toUpload = fileList.filter((_, i) => !existingIndices.includes(i));
    } catch (err) {
        toUpload = fileList;
    }

    // Phase 2: upload with retry support
    let ok = 0, fail = 0, firstErr = null;
    const failedFiles = [];

    for (let i = 0; i < toUpload.length; i++) {
        const f = toUpload[i];
        Dom.uploadStatus.textContent = `上传中 (${i + 1}/${toUpload.length}): ${f.name}`;
        try {
            await Api.uploadFile(f, 'local', (loaded, total) => {
                const pct = total > 0 ? Math.min(100, Math.round(loaded / total * 100)) : 30;
                Dom.uploadProgressFill.style.width = pct + '%';
                Dom.uploadProgressText.textContent = pct + '%';
            });
            ok++;
        } catch (err) {
            fail++;
            failedFiles.push(f);
            if (!firstErr) firstErr = err.message;
        }
    }

    // Phase 3: retry failed files once
    if (failedFiles.length > 0) {
        Dom.uploadStatus.textContent = `重试 ${failedFiles.length} 个失败文件...`;
        for (const f of failedFiles) {
            try {
                await Api.uploadFile(f, 'local', (loaded, total) => {
                    const pct = total > 0 ? Math.min(100, Math.round(loaded / total * 100)) : 30;
                    Dom.uploadProgressFill.style.width = pct + '%';
                    Dom.uploadProgressText.textContent = pct + '%';
                });
                ok++;
                fail--;
            } catch (err) {
                if (!firstErr) firstErr = err.message;
            }
        }
    }

    Dom.uploadOverlay.classList.add('hidden');
    Dom.uploadProgressFill.style.width = '0%';
    Dom.uploadProgressText.textContent = '0%';

    const summary = skipCount > 0
        ? `上传完成: ${ok} 个成功（${skipCount} 个已跳过）`
        : (ok > 0 ? `上传完成: ${ok} 个成功` : null);
    if (summary) {
        loadHomeFeed();
        alert(`${summary}${fail > 0 ? `, ${fail} 个失败` : ''}${firstErr ? `\n错误: ${firstErr}` : ''}`);
    } else {
        alert(`上传失败: ${firstErr || '未知错误'}`);
    }
}
