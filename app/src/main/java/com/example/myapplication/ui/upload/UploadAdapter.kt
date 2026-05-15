package com.example.myapplication.ui.upload

import android.content.res.Resources
import android.view.LayoutInflater
import android.view.ViewGroup
import androidx.core.content.ContextCompat
import androidx.recyclerview.widget.DiffUtil
import androidx.recyclerview.widget.ListAdapter
import androidx.recyclerview.widget.RecyclerView
import com.example.myapplication.R
import com.example.myapplication.databinding.ItemUploadBinding
import java.util.Locale

class UploadAdapter(
    private val onCheckedChange: ((position: Int, checked: Boolean) -> Unit)? = null
) : ListAdapter<UploadItem, UploadAdapter.ViewHolder>(DiffCallback) {

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val binding = ItemUploadBinding.inflate(LayoutInflater.from(parent.context), parent, false)
        return ViewHolder(binding)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        holder.bind(getItem(position), position)
    }

    inner class ViewHolder(
        private val binding: ItemUploadBinding
    ) : RecyclerView.ViewHolder(binding.root) {
        fun bind(item: UploadItem, position: Int) {
            val ctx = binding.root.context
            binding.fileName.text = item.fileName
            binding.fileSize.text = if (item.fileSize > 0) formatSize(item.fileSize) else "> SIZE: --"

            val progress = (item.progress * 1000).toInt().coerceIn(0, 1000)
            binding.progressBar.progress = progress

            val canSelect = item.status == UploadStatus.QUEUED
            binding.checkSelect.isEnabled = canSelect
            binding.checkSelect.setOnCheckedChangeListener(null)
            binding.checkSelect.isChecked = canSelect && item.selected
            if (canSelect) {
                binding.checkSelect.setOnCheckedChangeListener { _, isChecked ->
                    onCheckedChange?.invoke(position, isChecked)
                }
            }

            val statusText: String
            val statusColor: Int
            when (item.status) {
                UploadStatus.QUEUED -> {
                    statusText = "> QUEUED"
                    statusColor = ContextCompat.getColor(ctx, R.color.text_muted)
                    binding.progressBar.progress = 0
                }
                UploadStatus.CHECKING -> {
                    statusText = "> CHECKING..."
                    statusColor = ContextCompat.getColor(ctx, R.color.neon_amber)
                    binding.progressBar.progress = 0
                }
                UploadStatus.UPLOADING -> {
                    val rate = if (item.progress > 0) "${(item.progress * 100).toInt()}%" else ""
                    statusText = "> UPLOADING $rate"
                    statusColor = ContextCompat.getColor(ctx, R.color.neon_cyan)
                }
                UploadStatus.SUCCESS -> {
                    statusText = "> COMPLETE"
                    statusColor = ContextCompat.getColor(ctx, R.color.neon_green)
                    binding.progressBar.progress = 1000
                }
                UploadStatus.DUPLICATE -> {
                    statusText = "> SKIPPED (DUPLICATE)"
                    statusColor = ContextCompat.getColor(ctx, R.color.neon_amber)
                    binding.progressBar.progress = 1000
                }
                UploadStatus.FAILED -> {
                    statusText = "> FAILED: ${item.errorMessage?.take(40) ?: "UNKNOWN"}"
                    statusColor = ContextCompat.getColor(ctx, R.color.neon_red)
                }
            }
            binding.statusText.text = statusText
            binding.statusText.setTextColor(statusColor)
        }
    }

    companion object {
        private val DiffCallback = object : DiffUtil.ItemCallback<UploadItem>() {
            override fun areItemsTheSame(oldItem: UploadItem, newItem: UploadItem): Boolean =
                oldItem.id == newItem.id

            override fun areContentsTheSame(oldItem: UploadItem, newItem: UploadItem): Boolean =
                oldItem == newItem
        }

        fun formatSize(bytes: Long): String {
            if (bytes < 1024L) return "$bytes B"
            val k = 1024.0
            val kb = bytes / k
            if (kb < 1024.0) return String.format(Locale.US, "%.1f KB", kb)
            val mb = kb / 1024.0
            if (mb < 1024.0) return String.format(Locale.US, "%.1f MB", mb)
            return String.format(Locale.US, "%.2f GB", mb / 1024.0)
        }
    }
}
