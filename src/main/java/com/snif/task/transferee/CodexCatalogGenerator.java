package com.snif.task.transferee;

import cn.hutool.core.io.FileUtil;
import cn.hutool.json.JSONArray;
import cn.hutool.json.JSONObject;
import cn.hutool.json.JSONUtil;

import java.nio.charset.StandardCharsets;
import cn.hutool.core.io.FileUtil;
import cn.hutool.json.JSONArray;
import cn.hutool.json.JSONObject;
import cn.hutool.json.JSONUtil;

import java.nio.charset.StandardCharsets;

public class CodexCatalogGenerator {

    public static void main(String[] args) {
        // 官方 catalog
        String sourcePath =
                "C:\\Users\\14128\\.codex\\models_cache.json";

        // 最终生成 catalog
        String targetPath =
                "C:\\Users\\14128\\.codex\\codexmate\\relay\\codex_router_catalog.json";

        build(sourcePath, targetPath);
    }

    public static void build(String sourcePath, String targetPath) {
        JSONObject sourceRoot = JSONUtil.parseObj(
                FileUtil.readString(sourcePath, StandardCharsets.UTF_8)
        );

        JSONArray sourceModels = sourceRoot.getJSONArray("models");
        if (sourceModels == null || sourceModels.isEmpty()) {
            throw new IllegalStateException("原配置缺少 models 或 models 为空");
        }

        // 1. 提取原配置里面的 models
        JSONArray targetModels = new JSONArray();
        for (int i = 0; i < sourceModels.size(); i++) {
            targetModels.add(sourceModels.getJSONObject(i));
        }

        // 2. 从中提取第一项作为参考模板
        JSONObject template = sourceModels.getJSONObject(0);

        // 3. 基于模板改造一份自己的模型，追加进 models
        JSONObject customModel = JSONUtil.parseObj(template.toString());

        customModel.set("slug", "aimami_relay_17c5d6fd68");
        customModel.set("display_name", "cmy");
        customModel.set("description", "通过本地中转访问自定义模型");
        customModel.set("priority", 100);

        // 可选：避免官方 GPT-5.5 首次提示污染你的自定义模型
        customModel.set("availability_nux", null);

        targetModels.add(customModel);

        // 4. 输出目标文件，结构为 { "models": [ ... ] }
        JSONObject targetRoot = new JSONObject();
        targetRoot.set("models", targetModels);

        FileUtil.writeString(
                JSONUtil.toJsonPrettyStr(targetRoot),
                targetPath,
                StandardCharsets.UTF_8
        );

        System.out.println("生成完成：" + targetPath);
    }
}
