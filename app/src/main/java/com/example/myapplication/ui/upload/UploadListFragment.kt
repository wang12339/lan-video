package com.example.myapplication.ui.upload

import android.content.ContentResolver
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.LayoutInflater
import android.view.View
import android.view.ViewGroup
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.fragment.app.Fragment
import androidx.lifecycle.lifecycleScope
import androidx.navigation.fragment.findNavController
import androidx.recyclerview.widget.LinearLayoutManager
import com.example.myapplication.data.repository.VideoRepository
import com.example.myapplication.databinding.FragmentUploadListBinding
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class UploadListFragment : Fragment() {

    private var _binding: FragmentUploadListBinding? = null
    private val binding get() = _binding!!
    private val repository get() = VideoRepository.getInstance()
    private lateinit var adapter: UploadAdapter

    private var pendingUris: List<Uri> = emptyList()
    private var uploadItems: List<UploadItem> = emptyList()
    private var allPendingUris: List<Uri> = emptyList()
    private var uploadJob: Job? = null

    private val pickMoreVideos = registerForActivityResult(
        ActivityResultContracts.GetMultipleContents()
    ) { uris: List<Uri> ->
        if (uris.isNotEmpty()) {
            appendNewUploads(uris)
        }
    }

    override fun onCreateView(
        inflater: LayoutInflater, container: ViewGroup?, savedInstanceState: Bundle?
    ): View {
        _binding = FragmentUploadListBinding.inflate(inflater, container, false)
        return binding.root
    }

    override fun onViewCreated(view: View, savedInstanceState: Bundle?) {
        super.onViewCreated(view, savedInstanceState)

        // 恢复持久化的上传列表
        val savedUriList = savedInstanceState?.getParcelableArrayList<Uri>("allPendingUris")
        if (savedUriList != null) {
            allPendingUris = savedUriList
            val cr = requireContext().contentResolver
            uploadItems = allPendingUris.mapIndexed { index, uri ->
                val name = queryDisplayName(cr, uri) ?: "文件 ${index + 1}"
                val size = queryFileSize(cr, uri)
                UploadItem(uri = uri, fileName = name, fileSize = size, selected = true, originalIndex = index)
            }
        }

        // 从 arguments 恢复新传入的待上传列表
        arguments?.getParcelableArrayList<Uri>("pendingUris")?.let { uris ->
            this.pendingUris = uris
        }

        adapter = UploadAdapter(onCheckedChange = { position, checked ->
            if (position < uploadItems.size) {
                uploadItems = uploadItems.toMutableList().also {
                    it[position] = it[position].copy(selected = checked)
                }
                updateSelectAllButtonText()
            }
        })
        binding.recyclerUpload.layoutManager = LinearLayoutManager(requireContext())
        binding.recyclerUpload.adapter = adapter

        binding.btnBack.setOnClickListener {
            findNavController().navigateUp()
        }

        binding.btnSelectAll.setOnClickListener {
            toggleSelectAll()
        }

        binding.btnStartUpload.setOnClickListener {
            startUpload()
        }

        binding.btnAddMore.setOnClickListener {
            pickMoreVideos.launch("video/*")
        }

        ViewCompat.setOnApplyWindowInsetsListener(binding.toolbarArea) { v, insets ->
            val top = insets.getInsets(WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingStart, top, v.paddingEnd, v.paddingBottom)
            insets
        }

        adapter.submitList(uploadItems)
        updateEmptyState()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        if (allPendingUris.isNotEmpty()) {
            outState.putParcelableArrayList("allPendingUris", ArrayList(allPendingUris))
        }
    }

    override fun onResume() {
        super.onResume()
        // fragment 每次恢复时检查是否有待添加的文件（从相册多批选择后返回）
        if (pendingUris.isNotEmpty()) {
            val uris = pendingUris.toList()
            pendingUris = emptyList()
            appendNewUploads(uris)
        }
    }

    private fun appendNewUploads(uris: List<Uri>) {
        val cr = requireContext().contentResolver
        val startIndex = allPendingUris.size
        allPendingUris = allPendingUris + uris
        val newItems = uris.mapIndexed { i, uri ->
            val index = startIndex + i
            val name = queryDisplayName(cr, uri) ?: "文件 ${index + 1}"
            val size = queryFileSize(cr, uri)
            UploadItem(uri = uri, fileName = name, fileSize = size, selected = true, originalIndex = index)
        }
        uploadItems = uploadItems + newItems
        adapter.submitList(uploadItems)
        updateEmptyState()
        updateSelectAllButtonText()
    }

    private fun toggleSelectAll() {
        val hasUnselected = uploadItems.any { it.status == UploadStatus.QUEUED && !it.selected }
        val newSelected = if (hasUnselected) true else false
        uploadItems = uploadItems.map {
            if (it.status == UploadStatus.QUEUED) it.copy(selected = newSelected) else it
        }
        adapter.submitList(uploadItems)
        updateSelectAllButtonText()
    }

    private fun updateSelectAllButtonText() {
        val allSelected = uploadItems.filter { it.status == UploadStatus.QUEUED }
            .all { it.selected }
        binding.btnSelectAll.text = if (allSelected && uploadItems.any { it.status == UploadStatus.QUEUED }) {
            getString(com.example.myapplication.R.string.upload_select_none)
        } else {
            getString(com.example.myapplication.R.string.upload_select_all)
        }
    }

    private fun startUpload() {
        if (uploadJob?.isActive == true) return

        if (allPendingUris.isEmpty()) {
            Toast.makeText(requireContext(), com.example.myapplication.R.string.upload_no_selection, Toast.LENGTH_SHORT).show()
            return
        }

        val uris = allPendingUris.toList()

        // 显示上传开始反馈
        Toast.makeText(requireContext(), "开始上传 ${uris.size} 个文件...", Toast.LENGTH_SHORT).show()

        uploadJob = requireActivity().lifecycleScope.launch {
            try {
                val app = requireContext().applicationContext
                val result = withContext(Dispatchers.IO) {
                    repository.uploadVideosSequential(
                        context = app,
                        uris = uris,
                        onEachProgress = { _, _, _, _, _ -> },
                        onItemStatus = { index, statusStr, progress, error ->
                            // 用 originalIndex 查找条目位置，避免并发上传时 index 偏移
                            val pos = uploadItems.indexOfFirst { it.originalIndex == index }
                            if (pos >= 0) {
                                val st = when (statusStr) {
                                    "checking" -> UploadStatus.CHECKING
                                    "uploading" -> UploadStatus.UPLOADING
                                    "success" -> UploadStatus.SUCCESS
                                    "duplicate" -> UploadStatus.DUPLICATE
                                    "failed" -> UploadStatus.FAILED
                                    else -> UploadStatus.FAILED
                                }
                                if (st == UploadStatus.SUCCESS || st == UploadStatus.DUPLICATE) {
                                    // 上传完成即从列表中移除
                                    uploadItems = uploadItems.toMutableList().also { it.removeAt(pos) }
                                    adapter.submitList(uploadItems)
                                    updateEmptyState()
                                } else {
                                    updateItem(pos) { it.copy(status = st, progress = progress, errorMessage = error) }
                                }
                            }
                        }
                    )
                }
                if (isAdded) {
                    val success = result.successCount
                    val dup = result.duplicateCount
                    val fail = result.failCount
                    val parts = mutableListOf<String>()
                    if (success > 0) parts.add("成功 $success 个")
                    if (dup > 0) parts.add("跳过 $dup 个（重复）")
                    if (fail > 0) parts.add("失败 $fail 个")
                    val msg = parts.joinToString("，").ifEmpty { "上传完成" }
                    Toast.makeText(requireContext(), msg, Toast.LENGTH_LONG).show()
                    // 清理状态
                    allPendingUris = emptyList()
                    uploadItems = emptyList()
                    adapter.submitList(uploadItems)
                    updateEmptyState()
                    updateSelectAllButtonText()
                }
            } catch (e: Exception) {
                if (isAdded) {
                    Toast.makeText(requireContext(), "上传出错: ${e.message?.take(50)}", Toast.LENGTH_LONG).show()
                }
            }
        }
    }

    private fun updateItem(index: Int, transform: (UploadItem) -> UploadItem) {
        val mutable = uploadItems.toMutableList()
        if (index < mutable.size) {
            mutable[index] = transform(mutable[index])
            uploadItems = mutable
            adapter.submitList(uploadItems)
        }
    }

    private fun updateEmptyState() {
        val empty = uploadItems.isEmpty()
        binding.emptyText.visibility = if (empty) View.VISIBLE else View.GONE
        binding.recyclerUpload.visibility = if (empty) View.GONE else View.VISIBLE
        binding.emptyText.text = if (empty) "暂无上传任务" else ""
    }

    private fun queryDisplayName(cr: ContentResolver, uri: Uri): String? {
        if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
            cr.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val i = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (i >= 0) return c.getString(i)
                }
            }
        }
        return uri.lastPathSegment
    }

    private fun queryFileSize(cr: ContentResolver, uri: Uri): Long {
        if (uri.scheme == ContentResolver.SCHEME_CONTENT) {
            cr.query(uri, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val i = c.getColumnIndex(OpenableColumns.SIZE)
                    if (i >= 0 && !c.isNull(i)) return c.getLong(i).coerceAtLeast(0L)
                }
            }
        }
        return -1L
    }

    override fun onDestroyView() {
        _binding = null
        super.onDestroyView()
    }
}
