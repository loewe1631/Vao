(()=>{
'use strict';
if(window.__TAURI__){window.__TAURI__.core.invoke('report_frontend_state',{stage:'script-start'}).catch(()=>{});window.addEventListener('error',e=>window.__TAURI__.core.invoke('report_frontend_state',{stage:'initial-error:'+e.message}).catch(()=>{}));window.addEventListener('unhandledrejection',e=>window.__TAURI__.core.invoke('report_frontend_state',{stage:'initial-promise:'+String(e.reason)}).catch(()=>{}));}
const native=!!window.__TAURI__,barMode=new URLSearchParams(location.search).has('bar');
const ipc=(command,args={})=>native?window.__TAURI__.core.invoke(command,args):Promise.reject('Open this page in Vao.');
const initial={settings:{asr:{base:'',model:'',protocol:'openai_audio'},llm:{base:'',model:'',protocol:'openai_chat'},backup:{base:'',model:'',protocol:'openai_chat'},language:'auto',translation_language:'English',auto_input:true,dictionary:[],ui_language:'zh',theme:'system',trial_enabled:true,trial_consent_version:'',onboarding_complete:false,shortcuts:{record:'Control+Alt+Space',translate:'Control+Alt+KeyT',cancel:'Escape'}},profile:{revision:0,learning:true,personalization:true,rules:[],votes:[],undo:[]},history:[],version:'0.1.0-beta.4'};
let data=structuredClone(initial),page='home',snap={session:{phase:'idle'},hotkeys:false},lastResult='',lastPhase='',lastError='',lesson='dictate',demoStep=0,demoTimer=null,toastTimer=null,polling=false;
const words={
  "zh": {
    "home": "开始",
    "guide": "使用指南",
    "habits": "表达习惯",
    "history": "历史",
    "dictionary": "词典",
    "models": "模型连接",
    "settings": "设置",
    "about": "关于",
    "preview": "公开测试版",
    "ready": "准备就绪",
    "notConnected": "等待模型连接",
    "intro": "让表达，越来越像你。",
    "introText": "说出你的想法，Vao 帮你整理成可以直接使用的文字。通过你的明确反馈，逐步熟悉不同场景下的表达习惯。",
    "start": "开始录音",
    "stop": "结束录音",
    "cancel": "取消",
    "thinking": "思考中",
    "listening": "聆听中",
    "primaryKey": "主快捷键",
    "translateKey": "局部翻译",
    "shortcutTip": "在目标输入框按快捷键开始，说完再按一次。",
    "modelNotice": "可使用默认体验，或连接自己的语音与文字模型。",
    "getStarted": "连接模型",
    "dictate": "直接说",
    "selection": "选中再说",
    "translate": "局部翻译",
    "dictateText": "在输入框说出内容，整理后预览或输入。",
    "selectionText": "选中文字，按主快捷键，直接说问题或修改要求。答案留在声浪上方，不会跳回 Vao。",
    "translateText": "直接按翻译键开始翻译录音，也可在普通录音中切换需要翻译的片段。",
    "result": "本次结果",
    "source": "原始识别",
    "copy": "复制结果",
    "copied": "已复制",
    "review": "请复核",
    "complete": "已整理",
    "resultEmpty": "你的下一次表达，从这里开始。",
    "feedbackText": "这次排版，你更喜欢哪种？",
    "paragraph": "自然段",
    "numbered": "编号分项",
    "once": "仅本次，不学习",
    "feedbackSaved": "已记录。这次选择已形成当前场景的排版偏好，可随时撤销或清空。",
    "ruleApplied": "偏好已更新，将从下一次输入开始生效。",
    "habitsIntro": "从你明确选择的排版开始，逐步适应你的习惯。你始终可以暂停、撤销或清空。",
    "learn": "记录学习反馈",
    "learnDesc": "只有你明确选择的长期偏好才计入证据。",
    "personal": "应用已学偏好",
    "personalDesc": "关闭后采用通用排版，保留已有学习数据。",
    "noRules": "还没有形成长期偏好。\n完成输入后，选择你喜欢的排版即可开始积累。",
    "evidence": "次已记录选择",
    "revision": "偏好更新",
    "undo": "撤销上次学习",
    "forget": "清空学习数据",
    "forgetConfirm": "清空全部偏好、候选和证据？此操作无法撤销。",
    "historyIntro": "仅保留最近7天、最多200条文字结果。录音在本次处理后释放，不写入历史。",
    "clear": "清空历史",
    "clearConfirm": "清空文字历史？学习偏好仍会保留。",
    "historyEmpty": "还没有输入记录。",
    "dictionaryIntro": "添加经常使用的专有名词，帮助模型保留准确拼写。通用版本不预置个人术语。",
    "dictionaryLabel": "每行一个词条",
    "dictionaryHint": "最多100条。词条会随本次请求发送给你选择的模型。",
    "save": "保存",
    "saved": "已保存",
    "modelsIntro": "分别连接语音识别和文字整理服务。可以使用本地部署或兼容协议的云端服务。",
    "experience": "默认体验仅用于测试和体验。更好的效果和稳定性，建议使用自有云服务或本地模型。",
    "asr": "语音识别",
    "llm": "文字整理 · 主用",
    "backup": "文字整理 · 备用",
    "optional": "可选",
    "base": "服务地址",
    "key": "API Key",
    "keyPlaceholder": "留空保留当前密钥；本地无鉴权服务可留空",
    "model": "模型",
    "fetch": "获取模型",
    "chooseModel": "请先获取模型",
    "advanced": "高级选项",
    "manual": "手动填写模型名称",
    "protocol": "连接协议",
    "openaiChat": "OpenAI Chat Completions",
    "vaoRefine": "Vao 体验整理协议",
    "test": "测试文字连接",
    "testHint": "发送一条固定的改口例句，不发送历史或录音。",
    "asrTestHint": "模型列表仅用于发现。保存后请录制一段短语音验证实际识别。",
    "localAsr": "本地部署建议",
    "localAsrText": "可优先评估 Qwen3-ASR-1.7B；资源有限时评估0.6B。NVIDIA 可使用相应推理服务，Apple Silicon 可评估兼容MLX服务。本版要求服务提供 multipart /v1/audio/transcriptions，并返回JSON text字段；仅模型名称相同不代表协议兼容。",
    "https": "远程连接使用HTTPS，本机服务可使用HTTP。密钥保存在本机钥匙串。",
    "shortcutSettings": "快捷键与输入",
    "autoInput": "自动输入到原目标",
    "autoInputDesc": "默认开启。整理后回填原输入位置，不会发送消息；原窗口或输入位置失效时保留结果。关闭后先预览。",
    "translationLanguage": "翻译目标语言",
    "recognitionLanguage": "识别语言",
    "auto": "自动识别",
    "appearance": "外观",
    "system": "跟随系统",
    "light": "浅色",
    "dark": "深色",
    "permissions": "系统权限",
    "microphone": "麦克风",
    "accessibility": "辅助功能",
    "openSettings": "打开系统设置",
    "shortcutFailed": "快捷键未就绪。请检查是否被其他程序占用。",
    "guideIntro": "无需手动选择整理模式。下面的动画只演示操作，不录音、不发送请求。",
    "play": "播放演示",
    "replay": "重新播放",
    "demoSteps": [
      "放好光标",
      "开始说话",
      "结束录音",
      "等待整理",
      "检查结果"
    ],
    "selectionSteps": [
      "选中文字",
      "直接说修改要求",
      "结束录音",
      "等待处理",
      "查看处理结果"
    ],
    "translationSteps": [
      "开始录音",
      "先说原语言",
      "开启翻译片段",
      "说出需要翻译的话",
      "关闭翻译并继续说",
      "结束录音",
      "检查分段结果"
    ],
    "selectionNote": "提问会显示在声栏上方并复制答案，不替换原文；自动输入开启时，修改要求会尝试替换原选区。",
    "translationNote": "空闲时按翻译键，可直接开始翻译录音；录音中按翻译键开启或关闭局部翻译。按主快捷键结束整段录音。",
    "aboutText": "围绕可解释、可撤销的个人习惯适配构建。首版支持 Apple Silicon Mac，macOS 12 或更新版本。",
    "aboutSource": "产品方向源于结合多种开源项目使用经验改编而来。",
    "limits": "当前测试边界",
    "limitsText": "支持OpenAI音频转写、Chat Completions及Vao整理协议；并非所有模型通用适配。排版学习基于明确反馈；识别准确率与长期学习效果仍需真实使用验证。",
    "ack": "知道了",
    "busy": "正在处理…",
    "keySaved": "密钥已保存",
    "keyStored": "已保存密钥",
    "removeKey": "删除密钥",
    "keysIndependent": "API Key 保存在本机钥匙串，历史与排版偏好保存在本机。",
    "browser": "浏览器预览，实际录音需打开Vao应用。",
    "app": "应用",
    "scopeChat": "日常消息",
    "scopeTasks": "AI任务",
    "scopeTechnical": "技术记录",
    "feedbackDone": "此结果已记录学习反馈。",
    "pasteSent": "已发送输入；尚未验证目标接收",
    "alpha": "Mac 公开测试版",
    "failed": "处理失败",
    "unrefined": "文字整理未完成，以下保留原始识别。",
    "risks": "引用、数字或限制可能变化，请核对后复制。",
    "duration": "秒",
    "connected": "已配置连接",
    "done": "完成",
    "connectionSuccess": "连接测试返回：",
    "noSelection": "所选文字读取依赖目标应用的辅助功能支持；读取失败时不会猜测原文。",
    "editShortcut": "更改",
    "shortcutEditor": "输入你的快捷键",
    "shortcutKey": "按键",
    "shortcutModifiers": "组合修饰键",
    "shortcutSave": "保存并生效",
    "shortcutDiscard": "取消更改",
    "shortcutDefaults": "恢复默认快捷键",
    "shortcutSaved": "快捷键已更新，立即生效；重启后保留。",
    "shortcutHelp": "点击“录入按键”，按住 Command、Control、Option 等修饰键，再按另一个键；也可手动填写 Command+Shift+T、Fn 或 Fn+Space。Fn 单键松开时触发。系统占用的组合会在保存时提示。",
    "captureShortcut": "录入按键",
    "capturing": "请按键，松开后确认…",
    "captureEnded": "录入已结束，可重新录入或手动填写。",
    "fnPreset": "Fn 单键",
    "fnSpacePreset": "Fn＋空格",
    "fnHelp": "在其他应用中使用 Fn / 🌐 快捷键，需要允许 Vao 的辅助功能权限；本窗口录入不需要。若系统同时弹出表情或切换输入法，请在系统设置→键盘，把“按下 Fn / 🌐 键”改为“不执行任何操作”。这不会由 Vao 自动修改。",
    "fnPermission": "打开权限设置",
    "keyboardSettings": "打开键盘设置",
    "nativeUnavailable": "按键录入暂不可用，可手动填写快捷键。",
    "pause": "暂停",
    "previous": "上一步",
    "next": "下一步",
    "demoHint": "点击左侧任一步骤查看；也可播放、暂停，按自己的节奏学习。",
    "demoWindow": "目标应用 · 演示",
    "demoSpeaking": "你说的话",
    "revisionHelp": "仅实际学到的偏好变化时更新，开关操作不计入。",
    "dictionaryHow": "怎样使用词典",
    "dictionaryStep1": "把容易识别错的名字、产品名或术语，按希望输出的准确写法填入，每行一个。",
    "dictionaryStep2": "点击保存。下一次开始录音时，词条会提供给语音识别与文字整理模型。",
    "dictionaryStep3": "读一段包含该词的短句，检查输出。要移除词条，删掉对应行后再保存。",
    "dictionaryExample": "格式示例（不会自动加入你的词典）",
    "dictionaryLimits": "这里只填正确词条，不填整段提示词或“错误→正确”规则。词条是识别和整理的参考，不保证强制替换；效果取决于模型是否支持相关提示。",
    "dictionaryCount": "个词条",
    "dictionaryEmpty": "例如：Vao\nAcme Studio\n项目星河",
    "input_monitoring": "输入监控",
    "previewExplanation": "文字已生成。本次先预览，不会自动填入原窗口；你可以复制，或为后续录音开启自动输入。",
    "protectedPreview": "结果已保留。你可以复制使用；原输入位置不可用时，不会自动填入其他窗口。",
    "enableAutoInput": "后续录音自动输入",
    "autoInputEnabled": "已开启。下次录音会在目标校验通过后自动输入。",
    "failureHint": "请查看原因后重新录音；已识别的文字会保留在下方。",
    "captureReady": "正在录入，支持 Command 等组合键及 Fn；无需先授权全局监听。",
    "shortcutPending": "快捷键已保存，系统授权生效后会自动启用。",
    "permissionInactive": "尚未生效",
    "permissionRepairTitle": "开关已开，重启后仍未生效？",
    "permissionRepairText": "先关闭并重新打开系统设置，核对下方应用路径。仍不生效时，重新登记对应权限，再在系统设置中开启 Vao。重新登记会撤销该项现有授权，需要你重新允许；不会重置其他应用或麦克风权限。",
    "permissionRepair": "重新登记辅助功能",
    "permissionRepairConfirm": "重新登记此项权限？Vao 当前的这项授权会被撤销，之后需要你在系统设置重新开启。",
    "revealApp": "在访达显示当前应用",
    "permissionGranted": "已授权",
    "permissionMissing": "待授权",
    "permissionDenied": "未允许",
    "permissionUnknown": "待检测",
    "authorize": "授权",
    "refreshStatus": "重新检测",
    "permissionIntro": "这里显示当前进程实际生效的权限。系统开关开启后，返回 Vao 检测；输入监控通常需要退出并重新打开应用。",
    "microphoneHint": "用于录制你的声音。",
    "accessibilityHint": "用于 Fn 快捷键、读取选区，并将结果输入到原目标。",
    "inputHint": "输入监控用于系统键盘监听，按系统提示授权。当前 Fn 方案主要依赖辅助功能；无需为普通录音全部开启。授权后通常需要重启 Vao。",
    "legacyPermission": "请核对当前安装路径和 Vao 名称。同名的其他安装包获得授权，不代表正在运行的应用已获授权。",
    "trialTitle": "默认体验服务",
    "trialDisclosure": "体验服务用于在配置自有服务前测试 Vao 的功能。音频、待整理文字与本次词典会发送至 Vao 的体验服务器处理。共享服务无法保证响应速度；繁忙时请求会排队，输出可能变慢。为获得更稳定的响应和更好的效果，建议配置自有云端服务或本地模型。",
    "trialProvisioned": "本设备已开通",
    "trialUnprovisioned": "本设备尚未开通",
    "trialDisabled": "体验已关闭",
    "trialEnable": "使用默认体验",
    "trialEnableHint": "未配置自有连接的阶段使用体验服务；已有连接始终优先。",
    "trialSources": "当前处理方式",
    "customSource": "自有模型",
    "trialSource": "默认体验",
    "trialCheck": "检测服务",
    "trialTest": "测试整理",
    "trialChecking": "正在检测服务…",
    "trialCheckHint": "检测服务不发送录音；测试整理只发送一条固定改口例句。",
    "trialLimit": "每段语音最多 120 秒；每次识别、整理各占一次请求。额度按 UTC 自然日恢复。",
    "trialRestore": "恢复体验服务",
    "trialRestoreConfirm": "清除此阶段的自有连接设置，改用默认体验？已保存密钥仍留在钥匙串。",
    "backupClear": "停用备用",
    "trialUnavailable": "暂不可用",
    "trialAvailable": "服务可连接",
    "trialUnused": "尚未检测服务",
    "trialRemaining": "今日剩余请求",
    "trialExpires": "体验到期",
    "trialGlobal": "共享服务繁忙时会短暂排队；等待超时可稍后重试，或切换自有服务。",
    "setupTitle": "开始前的准备",
    "setupModels": "模型已就绪",
    "setupModelsMissing": "配置模型或开通体验",
    "setupMic": "麦克风待授权",
    "setupKeys": "快捷键待检查",
    "setupReady": "可以开始录音",
    "setupHelp": "先完成权限设置，然后在输入框按主快捷键说话。",
    "ownConnections": "自有模型",
    "ownConnectionsHint": "可以只替换语音识别或文字整理。自有连接出错时会明确提示，不自动切回体验。",
    "configuredReady": "连接配置已就绪",
    "needsPermissions": "请完成权限",
    "trialFailed": "检测失败",
    "requesting": "等待系统授权…",
    "connectionOk": "测试连接成功",
    "connectionFailed": "测试连接失败",
    "connectionTesting": "正在测试连接…",
    "connectionLatency": "延迟",
    "connectionInput": "测试原文",
    "connectionOutput": "模型返回",
    "connectionTimeHint": "从发出测试请求到完整返回的总耗时。",
    "permissionSteps": "1. 点击对应权限的“授权”，在系统设置开启 Vao。2. 列表没有 Vao 时，用下方“在访达显示当前应用”定位本应用，通过系统列表的＋号添加。3. 授权后点击“重启 Vao 并检测”。",
    "permissionPath": "正在运行的应用",
    "permissionRestart": "重启 Vao 并检测",
    "permissionRestartConfirm": "现在重启 Vao 以加载系统授权？未复制的预览结果请先复制；设置和已保存的历史会保留。",
    "permissionRepairInput": "重新登记输入监控",
    "permissionRequestHelp": "已向 macOS 请求权限。请在系统设置开启 Vao；列表没有时，可用“在访达显示当前应用”找到正在运行的安装包并添加。勾选后重启 Vao，再核对真实状态。",
    "setupPreviewOnly": "可以录音 · 跨应用权限待完成",
    "translationActive": "翻译中",
    "captureNextKey": "继续按下另一个键",
    "enrollTitle": "免费开通体验",
    "enrollConsent": "我已阅读并同意体验服务数据处理说明",
    "enrollPrivacy": "数据处理说明",
    "enrollDetails": "开通只提交随机安装标识与同意版本，不读取硬件编号。使用体验时，录音、当前文字、词典和本次处理所需偏好会通过 HTTPS 发送至 Vao 服务器处理；选区修改还会发送所选文字。服务不保存录音或文字正文，不用于训练；运行时短暂处理，保留不含内容的运维日志。设备凭据仅存本机钥匙串；服务保存凭据摘要、到期与额度，以及按日变化的来源网络摘要用于限流。自有连接直接访问你配置的服务，受其隐私政策约束。",
    "enrollLimits": "体验有效期 30 天，每日 200 个请求，普通一次录音通常使用 2 个。每日共享容量有限；开通或使用繁忙时可稍后重试，也可直接使用自有模型。",
    "enrolling": "正在开通…",
    "enrollSuccess": "体验已开通",
    "setupWelcome": "欢迎使用 Vao",
    "setupIntro": "完成下面三步，即可开始。可随时在设置中更改。",
    "setupOne": "1. 选择处理方式",
    "setupTwo": "2. 授予权限",
    "setupThree": "3. 选择输入方式",
    "setupChoose": "开通体验或连接自有模型",
    "setupFinish": "完成设置",
    "setupFinishHint": "先连接模型并授权麦克风。快捷键和跨应用输入权限可在设置中检查。",
    "setupDone": "首次设置已完成",
    "wechatLimit": "微信等不暴露输入框的应用，请先点击消息输入框；回填不会自动发送消息。",
    "inputManualHelp": "列表里没有 Vao？点击系统窗口左下角「＋」，选择「应用程序 → Vao」，添加后开启开关，再重启 Vao。"
  },
  "en": {
    "home": "Home",
    "guide": "How to use",
    "habits": "Your preferences",
    "history": "History",
    "dictionary": "Dictionary",
    "models": "Model connections",
    "settings": "Settings",
    "about": "About",
    "preview": "Public beta",
    "ready": "Ready",
    "notConnected": "Connect your models",
    "intro": "Your words. More like you.",
    "introText": "Speak your thoughts. Vao turns them into text you can use, and adapts to your preferences through explicit feedback.",
    "start": "Start recording",
    "stop": "Finish recording",
    "cancel": "Cancel",
    "thinking": "Thinking",
    "listening": "Listening",
    "primaryKey": "Main shortcut",
    "translateKey": "Translate a segment",
    "shortcutTip": "Press the shortcut in your input field. Press again when finished.",
    "modelNotice": "Use the built-in trial or connect your own speech and text models.",
    "getStarted": "Connect models",
    "dictate": "Speak directly",
    "selection": "Select, then speak",
    "translate": "Translate a segment",
    "dictateText": "Speak in an input field. Preview or insert the refined text.",
    "selectionText": "Select text, press the main shortcut, and speak your instruction immediately.",
    "translateText": "Start a translated recording directly, or toggle translation for part of a normal recording.",
    "result": "Latest result",
    "source": "Raw transcription",
    "copy": "Copy result",
    "copied": "Copied",
    "review": "Review needed",
    "complete": "Refined",
    "resultEmpty": "Your next thought starts here.",
    "feedbackText": "Which layout do you prefer here?",
    "paragraph": "Paragraphs",
    "numbered": "Numbered list",
    "once": "This time only",
    "feedbackSaved": "Recorded. This choice now sets the layout preference for this context. You can undo or clear it at any time.",
    "ruleApplied": "Preference updated. It applies from your next session.",
    "habitsIntro": "Start with explicit layout feedback. Pause, undo, or delete what Vao learns at any time.",
    "learn": "Collect learning feedback",
    "learnDesc": "Only preferences you explicitly choose to remember count as evidence.",
    "personal": "Apply learned preferences",
    "personalDesc": "Turn off to use defaults while keeping your learning data.",
    "noRules": "No lasting preferences yet.\nChoose a layout after dictation to start learning.",
    "evidence": "recorded choices",
    "revision": "Preference updates",
    "undo": "Undo last learned change",
    "forget": "Delete learning data",
    "forgetConfirm": "Delete all preferences, candidates, and evidence? This cannot be undone.",
    "historyIntro": "Text results are kept for 7 days, up to 200 entries. Audio is released after the session and is not saved in history.",
    "clear": "Clear history",
    "clearConfirm": "Clear text history? Learned preferences will remain.",
    "historyEmpty": "No input history yet.",
    "dictionaryIntro": "Add names and terms you use often. Public defaults contain no private vocabulary.",
    "dictionaryLabel": "One term per line",
    "dictionaryHint": "Up to 100 terms. Terms are sent to the models selected for a request.",
    "save": "Save",
    "saved": "Saved",
    "modelsIntro": "Connect speech recognition and text refinement separately, using local or compatible cloud services.",
    "experience": "The trial is for testing and exploration. Use your own cloud or local models for better quality and availability.",
    "asr": "Speech recognition",
    "llm": "Text refinement · Primary",
    "backup": "Text refinement · Backup",
    "optional": "Optional",
    "base": "Service URL",
    "key": "API key",
    "keyPlaceholder": "Leave blank to retain the key; unauthenticated local services need none",
    "model": "Model",
    "fetch": "Fetch models",
    "chooseModel": "Fetch models first",
    "advanced": "Advanced",
    "manual": "Enter model ID manually",
    "protocol": "Protocol",
    "openaiChat": "OpenAI Chat Completions",
    "vaoRefine": "Vao trial refinement",
    "test": "Test text connection",
    "testHint": "Sends one fixed correction example. No history or audio is sent.",
    "asrTestHint": "A model list only proves discovery. Save and record a short phrase to verify speech recognition.",
    "localAsr": "Local speech deployment",
    "localAsrText": "Consider Qwen3-ASR-1.7B, or 0.6B with fewer resources. Use a suitable NVIDIA inference server, or evaluate an MLX-compatible service on Apple Silicon. This beta requires multipart /v1/audio/transcriptions returning a JSON text field. A matching model name does not establish protocol support.",
    "https": "Use HTTPS for remote services. Local HTTP is supported. Keys are kept in your system keychain.",
    "shortcutSettings": "Shortcuts and input",
    "autoInput": "Insert into the original target",
    "autoInputDesc": "On by default. Refined text returns to the original input without sending the message. An unavailable target preserves the result. Turn off to preview first.",
    "translationLanguage": "Translation language",
    "recognitionLanguage": "Recognition language",
    "auto": "Auto-detect",
    "appearance": "Appearance",
    "system": "System",
    "light": "Light",
    "dark": "Dark",
    "permissions": "System permissions",
    "microphone": "Microphone",
    "accessibility": "Accessibility",
    "openSettings": "Open System Settings",
    "shortcutFailed": "Shortcuts are unavailable. Check whether another application uses them.",
    "guideIntro": "No manual refinement mode. These animations explain the workflow without recording or sending requests.",
    "play": "Play demo",
    "replay": "Replay",
    "demoSteps": [
      "Place your cursor",
      "Start speaking",
      "Finish recording",
      "Wait for refinement",
      "Review your text"
    ],
    "selectionSteps": [
      "Select the text",
      "Speak your instruction",
      "Finish recording",
      "Wait for processing",
      "View the result"
    ],
    "translationSteps": [
      "Start recording",
      "Speak normally",
      "Enable translation",
      "Speak the translated part",
      "Disable translation and continue",
      "Finish recording",
      "Review the segments"
    ],
    "selectionNote": "Questions appear above the voice bar and are copied without replacing the source. Edits try to replace the original selection when automatic insertion is enabled.",
    "translationNote": "When idle, the translation shortcut starts a translated recording. During recording, it turns segment translation on or off. Use the main shortcut to finish the recording.",
    "aboutText": "Built around explainable, reversible personal adaptation. This release supports Apple Silicon Mac running macOS 12 or later.",
    "aboutSource": "The product direction was shaped by experience using and adapting multiple open-source projects.",
    "limits": "Current limitations",
    "limitsText": "Supports OpenAI audio transcription, Chat Completions, and the Vao refinement protocol. Other protocols require adapters. Layout learning uses explicit feedback; accuracy and long-term adaptation need real-use evaluation.",
    "ack": "OK",
    "busy": "Working…",
    "keySaved": "Key saved",
    "keyStored": "Key stored",
    "removeKey": "Delete key",
    "keysIndependent": "API keys stay in Keychain. History and layout preferences stay on this Mac.",
    "browser": "Browser preview. Open the Vao app to record.",
    "app": "Application",
    "scopeChat": "Everyday messages",
    "scopeTasks": "AI tasks",
    "scopeTechnical": "Technical notes",
    "feedbackDone": "Learning feedback recorded for this result.",
    "pasteSent": "Paste sent; receipt has not been verified",
    "alpha": "Mac public beta",
    "failed": "Processing failed",
    "unrefined": "Refinement did not complete. The raw transcription is preserved below.",
    "risks": "Quotes, numbers, or constraints may have changed. Review before copying.",
    "duration": "sec",
    "connected": "Connections configured",
    "done": "Done",
    "connectionSuccess": "Connection test returned: ",
    "noSelection": "Reading selected text depends on the target application’s accessibility support. Missing text is never inferred.",
    "editShortcut": "Change",
    "shortcutEditor": "Enter your shortcut",
    "shortcutKey": "Key",
    "shortcutModifiers": "Modifiers",
    "shortcutSave": "Save and apply",
    "shortcutDiscard": "Cancel edit",
    "shortcutDefaults": "Restore default shortcuts",
    "shortcutSaved": "Shortcuts updated immediately and retained after restart.",
    "shortcutHelp": "Choose Record keys, hold Command, Control or Option, then press another key. Or type Command+Shift+T, Fn or Fn+Space. Fn alone fires on release. Unavailable combinations are reported when saving.",
    "captureShortcut": "Record keys",
    "capturing": "Press your keys, then release…",
    "captureEnded": "Capture ended. Record again or enter the shortcut manually.",
    "fnPreset": "Fn only",
    "fnSpacePreset": "Fn + Space",
    "fnHelp": "Using Fn / Globe globally requires Accessibility for Vao. Recording it inside this window does not. If macOS also opens emoji or switches input sources, set “Press Fn / Globe key to” to “Do Nothing” in Keyboard settings. Vao does not change this setting automatically.",
    "fnPermission": "Open permissions",
    "keyboardSettings": "Keyboard settings",
    "nativeUnavailable": "Key capture is unavailable. You can type the shortcut manually.",
    "pause": "Pause",
    "previous": "Previous",
    "next": "Next",
    "demoHint": "Select any step, or play and pause at your own pace.",
    "demoWindow": "Target application · Demo",
    "demoSpeaking": "What you say",
    "revisionHelp": "Only actual learned-preference changes count. Toggling switches does not.",
    "dictionaryHow": "How to use your dictionary",
    "dictionaryStep1": "Enter the exact spelling of names, products, or terms that are often misheard, one per line.",
    "dictionaryStep2": "Save. Starting with your next recording, terms are provided to your speech and refinement models.",
    "dictionaryStep3": "Dictate a short sentence containing the term and check the result. To remove a term, delete its line and save again.",
    "dictionaryExample": "Format example (not added automatically)",
    "dictionaryLimits": "Enter correct terms only, not long prompts or “wrong → right” replacement rules. Terms are guidance, not guaranteed replacements; support depends on the selected models.",
    "dictionaryCount": "terms",
    "dictionaryEmpty": "For example: Vao\nAcme Studio\nProject Aurora",
    "input_monitoring": "Input Monitoring",
    "previewExplanation": "Your text is ready in preview. Copy it, or enable automatic insertion for future recordings.",
    "protectedPreview": "Result preserved. You can copy it; if the original input is unavailable, it will not be inserted into another window.",
    "enableAutoInput": "Insert future recordings automatically",
    "autoInputEnabled": "Enabled. Future recordings will insert after target checks pass.",
    "failureHint": "Review the error and record again. Any available transcription is preserved below.",
    "captureReady": "Recording keys, including Command combinations and Fn. Global access is not required here.",
    "shortcutPending": "Shortcut saved. It will activate once system permission takes effect.",
    "permissionInactive": "Not active",
    "permissionRepairTitle": "Still inactive after enabling and restarting?",
    "permissionRepairText": "Close and reopen System Settings, then verify the app path below. If it still fails, re-register the affected permission and enable Vao again. Re-registering revokes this permission until you grant it again. Other apps and microphone access are unaffected.",
    "permissionRepair": "Register Accessibility again",
    "permissionRepairConfirm": "Re-register this permission? This revokes Vao’s current access for this permission; you must enable it again in System Settings.",
    "revealApp": "Reveal current app in Finder",
    "permissionGranted": "Granted",
    "permissionMissing": "Access needed",
    "permissionDenied": "Not allowed",
    "permissionUnknown": "Not checked",
    "authorize": "Allow access",
    "refreshStatus": "Check again",
    "permissionIntro": "These are permissions effective for this process. Return after enabling Vao in System Settings. Input Monitoring usually requires quitting and reopening the app.",
    "microphoneHint": "Records your voice.",
    "accessibilityHint": "Enables Fn shortcuts, reads selections, and inserts results into the original target.",
    "inputHint": "System keyboard monitoring, only when required. Fn mainly uses Accessibility in this build; ordinary recording does not need every permission. Restart Vao after granting access.",
    "legacyPermission": "Check the Vao name and running installation path. Permissions for another copy do not grant access to this copy.",
    "trialTitle": "Built-in trial",
    "trialDisclosure": "The trial lets you test Vao before connecting your own service. Audio, text and relevant vocabulary are sent to Vao’s trial servers. This shared service cannot guarantee response times. Busy periods may involve queues and slower output. For more consistent response times and better results, connect your own cloud service or local models.",
    "trialProvisioned": "This device is enrolled",
    "trialUnprovisioned": "This device is not enrolled",
    "trialDisabled": "Trial disabled",
    "trialEnable": "Use built-in trial",
    "trialEnableHint": "Used only for stages without a custom connection. Your connections take priority.",
    "trialSources": "Current routing",
    "customSource": "Your model",
    "trialSource": "Built-in trial",
    "trialCheck": "Check service",
    "trialTest": "Test refinement",
    "trialChecking": "Checking service…",
    "trialCheckHint": "Checking sends no audio. Testing refinement sends one fixed correction example.",
    "trialLimit": "Up to 120 seconds per recording. Recognition and refinement each count as one request. Quotas reset at midnight UTC.",
    "trialRestore": "Use trial instead",
    "trialRestoreConfirm": "Clear this stage’s custom connection and use the trial? The saved key remains in Keychain.",
    "backupClear": "Disable backup",
    "trialUnavailable": "Unavailable",
    "trialAvailable": "Service reachable",
    "trialUnused": "Service not checked",
    "trialRemaining": "Requests remaining today",
    "trialExpires": "Trial expires",
    "trialGlobal": "Requests may briefly queue when busy. If waiting times out, try again later or connect your own service.",
    "setupTitle": "Ready when you are",
    "setupModels": "Models ready",
    "setupModelsMissing": "Connect models or enroll in trial",
    "setupMic": "Microphone access needed",
    "setupKeys": "Check shortcuts",
    "setupReady": "Ready to record",
    "setupHelp": "Complete permission setup, then press your main shortcut in an input field.",
    "ownConnections": "Your models",
    "ownConnectionsHint": "Replace speech or text independently. Custom connection errors are shown directly, without silently switching to the trial.",
    "configuredReady": "Connections configured",
    "needsPermissions": "Complete permissions",
    "trialFailed": "Check failed",
    "requesting": "Waiting for system permission…",
    "connectionOk": "Connection test successful",
    "connectionFailed": "Connection test failed",
    "connectionTesting": "Testing connection…",
    "connectionLatency": "Latency",
    "connectionInput": "Test input",
    "connectionOutput": "Model response",
    "connectionTimeHint": "Total time from sending the test request to receiving the complete response.",
    "permissionSteps": "1. Choose Authorize and enable Vao in System Settings. 2. If Vao is missing, use Reveal this app below, then add that exact app with the system list’s + button. 3. Restart Vao and check again.",
    "permissionPath": "Running application",
    "permissionRestart": "Restart Vao and check",
    "permissionRestartConfirm": "Restart Vao to load system permissions? Copy any unsaved preview first. Settings and saved history will be kept.",
    "permissionRepairInput": "Re-register Input Monitoring",
    "permissionRequestHelp": "The request was sent to macOS. Enable Vao in System Settings. If missing, use Reveal this app to locate and add this exact installation. Restart Vao after enabling access, then check its effective status.",
    "setupPreviewOnly": "Ready to record · cross-app access incomplete",
    "translationActive": "Translating",
    "captureNextKey": "press another key to complete",
    "enrollTitle": "Activate free trial",
    "enrollConsent": "I have read and agree to the trial data processing notice",
    "enrollPrivacy": "Data processing notice",
    "enrollDetails": "Enrollment sends a random installation identifier and notice version, not hardware identifiers. Trial requests send audio, current text, vocabulary and relevant preferences over HTTPS to Vao servers; selection edits also send the selected text. Audio and text bodies are processed temporarily, not retained by the service or used for training. Operational logs contain no content. Device credentials stay in Keychain. The service stores credential hashes, expiry, usage and a daily changing network digest for rate limits. Custom connections go directly to your provider and follow its privacy policy.",
    "enrollLimits": "30-day trial, 200 requests per day. A normal recording usually uses two. Daily service capacity is shared and limited; retry later if busy or use your own models.",
    "enrolling": "Activating…",
    "enrollSuccess": "Trial activated",
    "setupWelcome": "Welcome to Vao",
    "setupIntro": "Three steps to get started. Change these choices any time in Settings.",
    "setupOne": "1. Choose your models",
    "setupTwo": "2. Grant access",
    "setupThree": "3. Choose how to insert",
    "setupChoose": "Activate trial or connect your models",
    "setupFinish": "Finish setup",
    "setupFinishHint": "Connect models and grant microphone access first. Check shortcuts and cross-app access in Settings.",
    "setupDone": "Setup complete",
    "wechatLimit": "For WeChat and apps that hide input controls, click the message field first. Vao never sends the message.",
    "inputManualHelp": "Vao missing from the list? Click + at the bottom left of System Settings, choose Applications → Vao, add it and turn the switch on. Then restart Vao."
  }
};

let shortcutEditing=null,captureArmed=false,captureTimer=null,captureEpoch=0,shortcutDraft=null;
const scrollPositions={};let renderedPage=null;
function shortcuts(){return {...initial.settings.shortcuts,...data.settings.shortcuts};}

const connectionReports={},connectionEpochs={};
function connectionReportHtml(slot,config){const r=connectionReports[slot];if(!r||r.config!==JSON.stringify(config))return '';if(r.status==='testing')return `<div class="connection-test pending" role="status">${t('connectionTesting')}</div>`;if(r.status==='failed')return `<div class="connection-test failed" role="status"><strong>${t('connectionFailed')}</strong><p>${esc(r.error)}</p></div>`;return `<div class="connection-test success" role="status"><div class="connection-test-head"><strong>${t('connectionOk')}</strong><span title="${esc(t('connectionTimeHint'))}">${t('connectionLatency')}：${esc(r.result.latency_ms)} ms</span></div><dl><dt>${t('connectionInput')}</dt><dd>${esc(r.result.input)}</dd><dt>${t('connectionOutput')}</dt><dd>${esc(r.result.output)}</dd></dl></div>`;}
function paintConnectionReport(slot){const e=document.querySelector(`[data-connection-result="${slot}"]`);if(e)e.innerHTML=connectionReportHtml(slot,cardConfig(slot));}
function invalidateConnectionReport(slot){connectionEpochs[slot]=(connectionEpochs[slot]||0)+1;delete connectionReports[slot];const e=document.querySelector(`[data-connection-result="${slot}"]`);if(e)e.innerHTML='';}
let serviceStatus=null,serviceError='',permissionBusy=false;

function onboarding(){if(data.settings.onboarding_complete)return '';return `<section class="card onboarding"><h2>${t('setupWelcome')}</h2><p>${t('setupIntro')}</p><div class="setting"><div><h3>${t('setupOne')}</h3><p>${t('experience')}</p></div><button data-page="models">${t('setupChoose')}</button></div><div class="setting"><div><h3>${t('setupTwo')}</h3><p>${t('permissionIntro')}</p></div><button data-page="settings">${t('permissions')}</button></div><div class="setting"><div><h3>${t('setupThree')}</h3><p>${t('autoInputDesc')}</p></div>${switcher('setup-auto-input',data.settings.auto_input)}</div><p class="muted">${t('wechatLimit')}</p><button class="primary" id="finish-setup" ${!routeReady()||permissionValue('microphone')!=='granted'?'disabled':''}>${t('setupFinish')}</button><p class="muted">${t('setupFinishHint')}</p></section>`;}
const permissionValue=k=>data.permissions?.[k]||'unknown';
function permissionLabel(k){if(k==='accessibility'&&permissionValue(k)==='not_granted')return t('permissionInactive');if(k==='input_monitoring'&&permissionValue(k)==='not_granted')return data.settings.ui_language==='en'?'Not granted · optional':'未授权 · 按需';return t({granted:'permissionGranted',denied:'permissionDenied',restricted:'permissionDenied',not_determined:'permissionMissing',not_granted:'permissionMissing',unknown:'permissionUnknown'}[permissionValue(k)]||'permissionUnknown');}
function routeReady(){return data.trial?.ready??!!(data.settings.asr.base&&data.settings.llm.base);}
function readiness(){if(!routeReady())return 'notConnected';if(permissionValue('microphone')!=='granted')return 'needsPermissions';if(!snap.hotkeys)return 'setupKeys';if(isWindows())return 'setupWindowsPreview';if(permissionValue('accessibility')!=='granted')return 'setupPreviewOnly';return 'setupReady';}
function readinessHtml(){return `<section class="readiness"><div><h3>${t('setupTitle')}</h3><p>${t(readiness())}</p></div><div class="row"><button data-page="settings">${t('permissions')}</button><button data-page="models">${t('models')}</button></div></section>`;}

function permissionRows(){return (isWindows()?['microphone']:['microphone','accessibility','input_monitoring']).map(k=>`<div class="setting permission-row"><div><h3>${t(k)}</h3><p>${t({microphone:'microphoneHint',accessibility:'accessibilityHint',input_monitoring:'inputHint'}[k])}</p></div><div class="permission-actions"><span class="permission-state ${permissionValue(k)==='granted'?'granted':'pending'}" data-permission-status="${k}">${permissionLabel(k)}</span><button ${k==='input_monitoring'?'aria-describedby="input-monitoring-help"':''} data-request-permission="${k}">${t(permissionValue(k)==='granted'||['denied','restricted'].includes(permissionValue(k))?'openSettings':'authorize')}</button>${k==='input_monitoring'?`<p class="permission-inline-help" id="input-monitoring-help">${t('inputManualHelp')}</p>`:''}</div></div>`).join('');}
function permissionWorkActive(){return ['preparing','listening','processing','delivering'].includes(snap.session.phase);}
function permissionsCard(){if(isWindows())return `<section class="card" id="permissions-card"><div class="card-head"><h2>${t('permissions')}</h2><button id="refresh-permissions">${t('refreshStatus')}</button></div><p>${t('permissionIntro')}</p><div id="permission-rows">${permissionRows()}</div><p>${t('permissionSteps')}</p><div class="row"><button data-permission="microphone">${t('openSettings')}</button><button id="reveal-application">${data.settings.ui_language==='en'?'Show application':'显示当前程序'}</button></div><p id="permission-request-help" class="muted" role="status"></p></section>`;return `<section class="card" id="permissions-card"><div class="card-head"><div><h2>${t('permissions')}</h2><p>${t('permissionIntro')}</p></div><button id="refresh-permissions">${t('refreshStatus')}</button></div><div id="permission-rows">${permissionRows()}</div><div class="notice"><p>${t('permissionSteps')}</p><p>${t('permissionPath')}：<span class="application-path">${esc(data.permissions?.application_path||'Vao.app')}</span></p><div class="row"><button id="reveal-application">${t('revealApp')}</button><button id="restart-permissions" ${permissionWorkActive()?'disabled':''}>${t('permissionRestart')}</button></div></div><p id="permission-request-help" class="muted" role="status"></p><details class="permission-repair"><summary>${t('permissionRepairTitle')}</summary><p>${t('permissionRepairText')}</p><div class="row"><button id="repair-accessibility" data-repair-permission="accessibility" ${permissionWorkActive()?'disabled':''}>${t('permissionRepair')}</button><button id="repair-input-monitoring" data-repair-permission="input_monitoring" ${permissionWorkActive()?'disabled':''}>${t('permissionRepairInput')}</button></div></details><details class="permission-legacy"><summary>${data.settings.ui_language==='en'?'About old permission entries':'为什么系统里还有旧名字？'}</summary><p>${t('legacyPermission')}</p></details></section>`;}
function trialSummary(){if(serviceError)return `<span class="pending">${t('trialFailed')} · ${esc(serviceError)}</span>`;if(!serviceStatus)return t('trialUnused');const d=serviceStatus.device;return `<span class="granted">${t('trialAvailable')}</span>${d?` · ${t('trialRemaining')} ${Math.max(0,d.daily_limit-d.used_today)} / ${d.daily_limit} · ${t('trialExpires')} ${new Date(d.expires*1000).toLocaleDateString(data.settings.ui_language==='en'?'en-US':'zh-CN')}`:''}`;}
function trialCard(){const a=data.trial||{};const source=k=>a[k+'_source']==='custom'?t('customSource'):t('trialSource');return `<section class="card trial-card"><div class="card-head"><h2>${t('trialTitle')}</h2><span class="badge">${t(a.provisioned?'trialProvisioned':'trialUnprovisioned')}</span></div><p>${t('trialDisclosure')}</p><details class="trial-privacy"><summary>${t('enrollPrivacy')}</summary><p>${t('enrollDetails')}</p></details>${!a.consented?`<div class="trial-enrollment"><p>${t('enrollLimits')}</p><label class="consent"><input type="checkbox" id="trial-consent"> ${t('enrollConsent')}</label><div class="actions"><button class="primary" id="enroll-trial" disabled>${t('enrollTitle')}</button></div></div>`:''}<div class="route-summary"><div><span>${t('asr')}</span><strong>${source('asr')}</strong></div><div><span>${t('llm')}</span><strong>${source('llm')}</strong></div></div><div class="setting"><div><h3>${t('trialEnable')}</h3><p>${t('trialEnableHint')}</p></div>${switcher('trial-enabled',data.settings.trial_enabled!==false)}</div><div class="row"><button id="check-trial" ${!a.provisioned?'disabled':''}>${t('trialCheck')}</button><button id="test-trial" ${!a.provisioned||!a.consented||data.settings.trial_enabled===false?'disabled':''}>${t('trialTest')}</button></div><p class="service-status" id="trial-status" role="status">${trialSummary()}</p><p class="muted">${t('trialCheckHint')}</p><p class="muted">${t('trialLimit')} ${t('trialGlobal')}</p></section>`;}
let permissionEpoch=0;
async function refreshPermissions(){if(!native||permissionBusy)return;const epoch=++permissionEpoch;const result=await ipc('permissions');if(epoch!==permissionEpoch||permissionBusy)return;data.permissions=result;snap.hotkeys=result.shortcuts_ready??snap.hotkeys;if(document.querySelector('#permission-rows')){document.querySelector('#permission-rows').innerHTML=permissionRows();bindPermissions();}updateLive();}
function bindPermissions(){action('[data-request-permission]',async e=>{const kind=e.dataset.requestPermission;if(permissionValue(kind)==='granted'||['denied','restricted'].includes(permissionValue(kind))){await ipc('open_permission',{kind});return;}permissionBusy=true;permissionEpoch++;e.disabled=true;e.textContent=t('requesting');try{data.permissions=await ipc('request_permission',{kind});if(data.permissions[kind]!=='granted'&&kind!=='microphone'){const help=document.querySelector('#permission-request-help');if(help)help.textContent=t('permissionRequestHelp');await ipc('open_permission',{kind});}}finally{permissionBusy=false;await refreshPermissions();}});}
async function checkTrial(){serviceError='';const label=document.querySelector('#trial-status');if(label)label.textContent=t('trialChecking');try{serviceStatus=await ipc('trial_status');}catch(e){serviceStatus=null;serviceError=String(e);}if(document.querySelector('#trial-status'))document.querySelector('#trial-status').innerHTML=trialSummary();}

function displayKey(value){if(isWindows())return value.split('+').map(k=>({control:'Ctrl',ctrl:'Ctrl',alt:'Alt',shift:'Shift',super:'Win',meta:'Win',escape:'Esc'}[k.toLowerCase()]||k.replace(/^Key/,'').replace(/^Digit/,''))).join('+');return value.split('+').map(k=>({control:'⌃',ctrl:'⌃',alt:'⌥',option:'⌥',shift:'⇧',super:'⌘',meta:'⌘',command:'⌘',escape:'Esc',fn:'Fn'}[k.toLowerCase()]||k.replace(/^Key/,'').replace(/^Digit/,''))).join(' ');}
function shortcutLabel(slot){return displayKey(shortcuts()[slot]);}
function shortcutRow(slot,label,help){return `<div class="setting"><div><h3>${t(label)}</h3><p>${help?t(help):''}</p></div><div class="shortcut-value"><kbd data-shortcut-label="${slot}">${esc(shortcutLabel(slot))}</kbd><button data-edit-shortcut="${slot}">${t('editShortcut')}</button></div></div>${shortcutEditing===slot?shortcutEditor(slot):''}`;}
function shortcutEditor(slot){return `<div class="shortcut-editor" data-editor="${slot}"><h3>${t('shortcutEditor')}</h3><div class="row"><input id="shortcut-input" aria-label="${t('shortcutEditor')}" value="${esc(shortcutDraft??shortcuts()[slot])}" placeholder="${isWindows()?'Control+Alt+Space':'Fn / Fn+Space / Command+Shift+T'}" autocomplete="off" spellcheck="false"><button id="capture-shortcut">${t('captureShortcut')}</button></div>${isWindows()?'':`<div class="row shortcut-presets"><button data-shortcut-preset="Fn">${t('fnPreset')}</button><button data-shortcut-preset="Fn+Space">${t('fnSpacePreset')}</button></div>`}<p class="muted" id="capture-status" role="status"></p><div class="row"><button class="primary" id="save-shortcut">${t('shortcutSave')}</button><button id="cancel-shortcut">${t('shortcutDiscard')}</button></div><p class="muted fn-note">${t('fnHelp')}</p><div class="row">${isWindows()?'':`<button class="quiet" data-permission="accessibility">${t('fnPermission')}</button>`}<button class="quiet" data-permission="keyboard">${t('keyboardSettings')}</button></div></div>`;}
async function stopCapture(){captureEpoch++;captureArmed=false;clearTimeout(captureTimer);if(native)await ipc('shortcut_capture',{enabled:false});const button=document.querySelector('#capture-shortcut');if(button)button.textContent=t('captureShortcut');}
function showCaptureProgress(value){if(!captureArmed)return;const input=document.querySelector('#shortcut-input');if(input)input.value=value;const status=document.querySelector('#capture-status');if(status)status.textContent=value?displayKey(value)+' · '+t('captureNextKey'):t('captureReady');}
function acceptCaptured(value){if(!captureArmed||!shortcutEditing)return;shortcutDraft=value;const input=document.querySelector('#shortcut-input');if(input){input.value=value;input.blur();}stopCapture().catch(toast);const status=document.querySelector('#capture-status');if(status)status.textContent=displayKey(value);}
const isWindows=()=>data.platform==='windows';
const windowsWords={zh:{pasteSent:'已核对输入内容（未自动发送）',wechatLimit:'浏览器和聊天等尚不支持的输入框会保留结果供复制，不会自动发送。',windowsNotice:'Windows 预览：支持录音、整理、翻译与原生文本框回填。其他输入框保留结果供复制；请在笔记本实测。',setupWindowsPreview:'可开始录音 · 原生文本框可自动输入',autoInputDesc:'开启后尝试回填开始录音时的原生文本框（如记事本）。内容或光标改变、目标不支持时保留结果，不自动发送。',previewExplanation:'自动输入已关闭。请复制浮层或历史中的结果。',protectedPreview:'无法验证原输入位置，结果已保留，请复制。',shortcutHelp:'点击“录入按键”，按住 Ctrl、Alt 或 Shift，再按另一个键；也可手动填写 Control+Alt+Space。保存时检查系统占用。',fnHelp:'Windows 硬件 Fn 不作为快捷键；请使用 Ctrl、Alt、Shift 等普通组合键。',captureReady:'按住 Ctrl、Alt 或 Shift，再按另一个键。',microphoneHint:'点击检查时短暂打开麦克风，音频只在内存中丢弃；访问失败请打开 Windows 麦克风隐私设置。',permissionGranted:'访问检查已通过',permissionMissing:'尚未检查访问',permissionIntro:'检查实际麦克风访问，并确认 Windows 允许桌面应用使用麦克风。',permissionSteps:'Windows 设置 → 隐私和安全性 → 麦克风，打开麦克风及桌面应用访问。状态来自本次运行中的设备检查，实际录音仍需实测。',keysIndependent:'API Key 保存在 Windows 凭据管理器，历史与排版偏好保存在本机。',https:'远程连接使用 HTTPS，本机服务可使用 HTTP。密钥保存在 Windows 凭据管理器。',aboutText:'Windows x64 开发预览。请在已登录的桌面实测麦克风、快捷键与浮层。原生文本框支持验证后回填，其他输入框请复制结果。',trialRestoreConfirm:'清除此阶段的自有连接设置，改用默认体验？已保存密钥仍保存在 Windows 凭据管理器。'},en:{pasteSent:'Input contents verified; message not sent',wechatLimit:'Unsupported browser and chat fields keep results for copying. Messages are never sent automatically.',windowsNotice:'Windows preview: recording, refinement, translation and native text-field insertion. Other fields keep results for copying. Test on your PC.',setupWindowsPreview:'Ready to record · native text fields support insertion',autoInputDesc:'Try the starting native text field (such as Notepad). Changed content/caret or unsupported fields keep the result. Never sends messages.',previewExplanation:'Automatic insertion is off. Copy the result from the overlay or History.',protectedPreview:'The original input could not be verified. Copy the saved result.',shortcutHelp:'Choose Record keys, hold Ctrl, Alt or Shift, then press another key. Or type Control+Alt+Space. Conflicts are reported on save.',fnHelp:'Hardware Fn is not supported on Windows. Use a normal Ctrl, Alt or Shift combination.',captureReady:'Hold Ctrl, Alt or Shift, then press another key.',microphoneHint:'The check briefly opens the microphone and discards in-memory samples. Open Windows microphone privacy settings if access fails.',permissionGranted:'Access check passed',permissionMissing:'Access not checked',permissionIntro:'Check actual microphone access and allow desktop apps in Windows microphone privacy settings.',permissionSteps:'Windows Settings → Privacy & security → Microphone: allow microphone and desktop app access. Status reflects a device check in this run; recording still needs a desktop test.',keysIndependent:'API keys stay in Windows Credential Manager. History and preferences stay on this PC.',https:'Use HTTPS for remote services. Local HTTP is supported. Keys stay in Windows Credential Manager.',aboutText:'Windows x64 development preview. Test microphone, shortcuts and overlay on a logged-in desktop. Native text fields support verified insertion. Copy results for other fields.',trialRestoreConfirm:'Clear this custom connection and use the trial? Its saved key remains in Windows Credential Manager.'}};
const t=k=>{const lang=data.settings.ui_language==='en'?'en':'zh';let value=(isWindows()?windowsWords[lang][k]:undefined)??words[lang][k]??k;return isWindows()&&typeof value==='string'?value.replaceAll('本机钥匙串','Windows 凭据管理器').replaceAll('Keychain','Windows Credential Manager'):value;};
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
document.addEventListener('contextmenu',event=>{if(!event.target.closest('input,textarea,[contenteditable="true"]'))event.preventDefault();});
document.addEventListener('dragstart',event=>{if(!event.target.closest('input,textarea,.output,.original'))event.preventDefault();});
document.addEventListener('mousedown',event=>{if(barMode&&native&&event.button===0&&event.target.closest('[data-bar-drag]')){event.preventDefault();ipc('drag_voicebar').catch(()=>{});}});
document.addEventListener('keydown',event=>{
 if(!captureArmed||event.repeat||event.isComposing)return;
 event.preventDefault();event.stopPropagation();
 if(['Control','Alt','Shift','Meta','Fn'].includes(event.key)){showCaptureProgress([event.ctrlKey?'Control':'',event.altKey?'Alt':'',event.shiftKey?'Shift':'',event.metaKey?'Super':'',event.key==='Fn'?'Fn':''].filter(Boolean).join('+'));return;}
 const mods=[];if(event.ctrlKey)mods.push('Control');if(event.altKey)mods.push('Alt');if(event.shiftKey)mods.push('Shift');if(event.metaKey)mods.push('Super');if(event.getModifierState?.('Fn'))mods.push('Fn');
 if(event.code&&event.code!=='Unidentified')acceptCaptured([...mods,event.code].join('+'));
},true);
const icon=k=>{const d={home:'M3 10 12 3l9 7v10H4V10 M9 20v-7h6v7',guide:'M4 4h6l2 2 2-2h6v15h-6l-2 2-2-2H4Z M12 6v15',habits:'M4 7h16 M4 17h16 M8 4v6 M16 14v6',history:'M4 5v5h5 M4 10a8 8 0 1 1 0 6 M12 7v5l3 2',dictionary:'M5 4h14v17l-7-4-7 4Z',models:'M8 4h8v5H8Z M3 16h6v5H3Z M15 16h6v5h-6Z M12 9v4 M6 16v-3h12v3',settings:'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1Z',about:'M12 10v7 M12 6v1 M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20'}[k];return `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linejoin="round" stroke-linecap="round" aria-hidden="true"><path d="${d}"/></svg>`};
function toast(message){clearTimeout(toastTimer);document.querySelector('.toast')?.remove();const e=document.createElement('div');e.className='toast';e.role='status';e.textContent=String(message);document.body.append(e);toastTimer=setTimeout(()=>e.remove(),6500);}
function theme(){document.documentElement.dataset.theme=data.settings.theme;document.documentElement.lang=data.settings.ui_language==='en'?'en':'zh-CN';}
const scope=s=>t({chat:'scopeChat',tasks:'scopeTasks',technical:'scopeTechnical'}[s]||'scopeChat');
const switcher=(id,on)=>`<button class="switch" role="switch" id="${id}" aria-checked="${!!on}"></button>`;
const wave=()=>'<span class="wave" aria-hidden="true">'+Array.from({length:7},()=>'<i></i>').join('')+'</span>';
function render(){const oldScroll=document.querySelector(".scroll");if(oldScroll&&renderedPage)scrollPositions[renderedPage]=oldScroll.scrollTop;theme();if(barMode){document.body.className='bar-mode'+(isWindows()?' windows-bar':'');document.querySelector('#app').innerHTML=`<div class="bar-stage"><section id="bar-notice" class="bar-notice hidden"><div class="answer-toolbar hidden"><span class="answer-handle" data-bar-drag title="${data.settings.ui_language==='en'?'Drag to move':'拖动调整位置'}">${isWindows()?(data.settings.ui_language==='en'?'Result':'处理结果'):(data.settings.ui_language==='en'?'Answer':'追问结果')}</span>${isWindows()?`<button id="bar-copy">${t('copy')}</button>`:''}<button id="bar-dismiss" aria-label="${data.settings.ui_language==='en'?'Close answer':'关闭结果'}">×</button></div><div id="bar-delivery-hint" class="hidden" role="status"></div><div id="bar-message" role="status" aria-live="polite"></div></section><div class="voicebar" data-bar-drag title="${data.settings.ui_language==='en'?'Drag to move':'拖动调整位置'}"><span id="bar-label">${t('listening')}</span>${wave()}</div></div>`;document.querySelector('#bar-copy')?.addEventListener('click',async()=>{try{await ipc('copy_result',{id:snap.result?.id||0});document.querySelector('#bar-copy').dataset.copied=String(snap.session.id)+':'+String(snap.result?.id||0);updateLive();}catch(error){toast(error);}});document.querySelector('#bar-dismiss').addEventListener('click',async()=>{if(native){await ipc('dismiss_answer');await poll();}});updateLive();return;}
 const nav=['home','guide','habits','history','dictionary','models','settings','about'];document.querySelector('#app').innerHTML=`<div class="shell"><aside class="sidebar"><div class="brand"><img src="assets/logo.svg" alt="Vao"><div><strong>Vao</strong><span>BETA</span></div></div><nav class="nav" aria-label="Navigation">${nav.map(k=>`<button data-page="${k}" class="${page===k?'active':''}" ${page===k?'aria-current="page"':''}>${icon(k)}${t(k)}</button>`).join('')}</nav><div class="sidebar-foot">${t('alpha')}<br>${esc(data.version)}</div></aside><div class="workspace"><header class="toolbar"><strong>${t(page)}</strong><div class="toolbar-right"><span class="muted"><i class="status-dot"></i><span id="top-status">${t(readiness())}</span></span><button id="lang">${data.settings.ui_language==='en'?'中文':'EN'}</button><button id="theme" aria-label="${t('appearance')}">${t(data.settings.theme)}</button></div></header><main class="scroll"><div class="page">${isWindows()?`<div class="notice windows-preview" role="status">${t('windowsNotice')}<br>${esc(data.version)} / ${esc(data.build||'')}</div>`:''}${content()}</div></main></div></div>`;bind();renderedPage=page;document.querySelector(".scroll").scrollTop=scrollPositions[page]||0;updateLive();}
function content(){switch(page){case'home':return home();case'guide':return guide();case'models':return models();case'habits':return habits();case'history':return history();case'dictionary':return dictionary();case'settings':return settings();case'about':return about();}}
function home(){const card=`<section class="card" id="result-card"><div class="card-head"><h2>${t('result')}</h2><span class="badge">${t('preview')}</span></div><div id="result-content">${resultHtml(snap.result)}</div></section>`;const prominent=!!snap.result||snap.session.phase==='failed';return `${prominent?card:''}${onboarding()}<div class="hero"><div class="eyebrow">VAO / YOUR VOICE, YOUR WAY</div><h1>${t('intro')}</h1><p class="intro">${t('introText')}</p><div class="hero-actions"><button class="primary" id="record">${t('start')}</button><span class="shortcut"><kbd>${esc(shortcutLabel("record"))}</kbd></span></div><p class="muted">${t('shortcutTip')}</p></div><div id="live" class="live hidden"><div class="voicebar"><span id="live-label"></span>${wave()}</div><button id="translate">${t('translateKey')} <kbd>${esc(shortcutLabel("translate"))}</kbd></button><button id="cancel">${t('cancel')}</button></div>${readinessHtml()}<div class="grid">${(isWindows()?['dictate','translate']:['dictate','selection','translate']).map((k,i)=>`<div class="feature"><div class="num">0${i+1}</div><h3>${t(k)}</h3><p>${t(k+'Text')}</p></div>`).join('')}</div>${!prominent?card:''}`;}
function resultHtml(r){const failure=snap.session.phase==='failed'?`<div class="notice warn session-error" role="alert"><strong>${t('failed')}</strong><p>${esc(snap.error||t('failureHint'))}</p><p>${t('failureHint')}</p></div>`:'';if(!r)return failure||`<div class="empty">${t('resultEmpty')}</div>`;return `${failure}${r.risks.includes('unrefined')?`<div class="notice warn">${t('unrefined')}</div>`:''}<div class="output" tabindex="0">${esc(r.text)}</div>${r.delivery==='preview'?`<div class="preview-explanation"><p>${t(data.settings.auto_input?'protectedPreview':'previewExplanation')}</p>${!data.settings.auto_input?`<button data-enable-auto-input>${t('enableAutoInput')}</button>`:''}</div>`:''}<div class="row between"><span class="muted">${scope(r.scene)} · ${t('revision')} ${r.revision}${r.delivery==='paste_sent'?' · '+t('pasteSent'):''}</span><button data-copy="${r.id}">${t('copy')}</button></div><details class="muted"><summary>${t('source')}</summary><div class="original">${esc(r.original)}</div></details>${r.id>0&&!r.risks.includes('unrefined')?`<div class="feedback"><span class="muted">${t('feedbackText')}</span>${r.feedback?`<span class="muted">${t('feedbackDone')}</span>`:`<button data-feedback="paragraph" data-result="${r.id}">${t('paragraph')}</button><button data-feedback="numbered" data-result="${r.id}">${t('numbered')}</button><button data-feedback="once" data-result="${r.id}" class="quiet">${t('once')}</button>`}</div>`:''}`;}
function guide(){return `<h1>${t('guide')}</h1><p class="intro">${t('guideIntro')}</p><div class="steps">${(isWindows()?['dictate','translate']:['dictate','selection','translate']).map(k=>`<button data-lesson="${k}" class="${lesson===k?'active':''}">${t(k)}</button>`).join('')}</div><div class="card lesson-card"><div class="lesson-layout"><nav class="lesson-timeline" aria-label="${t('guide')}">${lessonSteps().map((label,i)=>`<button data-demo-step="${i}"><span>${i+1}</span><strong>${label}</strong></button>`).join('')}</nav><div id="demo" class="demo"></div></div><div class="row demo-controls"><button class="primary" id="demo-play">${t(demoTimer?'pause':'play')}</button><button id="demo-prev">${t('previous')}</button><button id="demo-next">${t('next')}</button><button class="quiet" id="demo-replay">${t('replay')}</button><span id="demo-progress" class="muted"></span></div><p class="muted">${t('demoHint')}</p></div><div class="notice">${lesson==='selection'?t('selectionNote'):lesson==='translate'?t('translationNote'):t('shortcutTip')}</div><div class="card"><div class="setting"><div><h3>${t('primaryKey')}</h3><p>${t('dictateText')}</p></div><kbd>${esc(shortcutLabel('record'))}</kbd></div><div class="setting"><div><h3>${t('translateKey')}</h3><p>${t('translationNote')}</p></div><kbd>${esc(shortcutLabel('translate'))}</kbd></div><div class="setting"><h3>${t('cancel')}</h3><kbd>${esc(shortcutLabel('cancel'))}</kbd></div></div>`;}
function modelCard(slot){const c=data.settings[slot];return `<section class="card model-card" data-slot="${slot}"><div class="card-head"><div><h2>${t(slot)}</h2><p>${t(slot==='asr'?'asrTestHint':'testHint')}</p></div>${slot==='backup'?`<span class="badge">${t('optional')}</span>`:''}</div><div class="fields"><div class="field"><label for="${slot}-base">${t('base')}</label><input id="${slot}-base" data-field="base" placeholder="https://api.example.com/v1" value="${esc(c.base)}" spellcheck="false" autocomplete="off"></div><div class="field"><label for="${slot}-key">${t('key')}</label><input id="${slot}-key" data-field="key" type="password" placeholder="${t('keyPlaceholder')}" autocomplete="new-password"></div></div><div class="field"><label for="${slot}-model">${t('model')}</label><div class="row"><select id="${slot}-model" data-field="model">${c.model?`<option value="${esc(c.model)}">${esc(c.model)}</option>`:`<option value="">${t('chooseModel')}</option>`}</select><button data-fetch="${slot}">${t('fetch')}</button></div></div><details><summary>${t('advanced')}</summary><div class="field"><label for="${slot}-manual">${t('manual')}</label><input id="${slot}-manual" data-field="manual" value="${esc(c.model)}" spellcheck="false"></div>${slot!=='asr'?`<div class="field"><label>${t('protocol')}</label><select data-field="protocol"><option value="openai_chat" ${c.protocol==='openai_chat'?'selected':''}>${t('openaiChat')}</option><option value="vao_refine" ${c.protocol==='vao_refine'?'selected':''}>${t('vaoRefine')}</option></select></div>`:''}<button data-remove-key="${slot}" class="quiet danger">${t('removeKey')}</button></details><div class="actions"><button class="primary" data-save-connection="${slot}">${t('save')}</button>${slot!=='asr'?`<button data-test="${slot}">${t('test')}</button>`:''}${c.base||c.model?`<button class="quiet" data-restore-trial="${slot}">${t(slot==='backup'?'backupClear':'trialRestore')}</button>`:''}<span class="muted" data-connection-note="${slot}"></span></div><div data-connection-result="${slot}">${connectionReportHtml(slot,c)}</div></section>`;}
function models(){return `<h1>${t('models')}</h1><p class="intro">${t('modelsIntro')}</p>${trialCard()}<div class="section-heading"><h2>${t('ownConnections')}</h2><p>${t('ownConnectionsHint')}</p></div>${['asr','llm','backup'].map(modelCard).join('')}<div class="card"><h2>${t('localAsr')}</h2><p>${t('localAsrText')}</p><p class="muted" style="margin-top:15px">${t('https')}</p></div>`;}

function habits(){const p=data.profile;return `<h1>${t('habits')}</h1><p class="intro">${t('habitsIntro')}</p><div class="card"><div class="setting"><div><h3>${t('learn')}</h3><p>${t('learnDesc')}</p></div>${switcher('learning',p.learning)}</div><div class="setting"><div><h3>${t('personal')}</h3><p>${t('personalDesc')}</p></div>${switcher('personalization',p.personalization)}</div></div><div class="card"><div class="row between"><h2>${t('habits')}</h2><span class="badge">${t('revision')} ${p.learned_revision??0}</span></div>${p.rules.length?p.rules.map(r=>`<div class="rule"><strong>${scope(r.scope)} · ${t(r.layout)}</strong><p>${r.evidence} ${t('evidence')}</p></div>`).join(''):`<div class="empty" style="white-space:pre-line">${t('noRules')}</div>`}<p class="muted">${t("revisionHelp")}</p><div class="row"><button id="undo" ${!p.undo.length?'disabled':''}>${t('undo')}</button><button id="forget" class="quiet danger">${t('forget')}</button></div></div>`;}
function history(){return `<div class="row between"><h1>${t('history')}</h1><button class="quiet danger" id="clear-history">${t('clear')}</button></div><p>${t('historyIntro')}</p><div class="card">${data.history.length?data.history.map(r=>`<article class="history-item"><div class="row between"><span class="muted">${new Date(r.created*1000).toLocaleString(data.settings.ui_language==='en'?'en-US':'zh-CN')} · ${scope(r.scene)}</span><button data-copy="${r.id}">${t('copy')}</button></div><div class="output">${esc(r.text)}</div></article>`).join(''):`<div class="empty">${t('historyEmpty')}</div>`}</div>`;}
function dictionary(){return `<h1>${t('dictionary')}</h1><p>${t('dictionaryIntro')}</p><div class="card dictionary-help"><h2>${t('dictionaryHow')}</h2><ol>${['dictionaryStep1','dictionaryStep2','dictionaryStep3'].map(k=>`<li>${t(k)}</li>`).join('')}</ol><div class="dictionary-example"><div><h3>${t('dictionaryExample')}</h3><pre>Vao\nAcme Studio\n${data.settings.ui_language==='en'?'Project Aurora':'项目星河'}</pre></div><p>${t('dictionaryLimits')}</p></div></div><div class="card"><div class="field"><label for="dictionary">${t('dictionaryLabel')}</label><textarea id="dictionary" rows="7" placeholder="${esc(t('dictionaryEmpty'))}">${esc(data.settings.dictionary.join('\n'))}</textarea><small><span id="dictionary-count">${data.settings.dictionary.length}</span> / 100 ${t('dictionaryCount')} · ${t('dictionaryHint')}</small></div><button class="primary" id="save-dictionary" style="margin-top:20px">${t('save')}</button></div>`;}
function settings(){return `<h1>${t('settings')}</h1><div class="card"><h2>${t('shortcutSettings')}</h2>${shortcutRow('record','primaryKey','shortcutTip')}${shortcutRow('translate','translateKey','translationNote')}${shortcutRow('cancel','cancel','')}<p class="muted">${t('shortcutHelp')}</p><button id="reset-shortcuts" class="quiet">${t('shortcutDefaults')}</button>${!snap.hotkeys?`<div class="notice warn">${t('shortcutFailed')}</div>`:''}<div class="setting"><div><h3>${t('autoInput')}</h3><p>${t('autoInputDesc')}</p></div>${switcher('auto-input',data.settings.auto_input)}</div><div class="setting"><h3>${t('recognitionLanguage')}</h3><select id="recognition-language">${[['auto',t('auto')],['zh','中文'],['en','English']].map(([v,l])=>`<option value="${v}" ${data.settings.language===v?'selected':''}>${l}</option>`).join('')}</select></div><div class="setting"><h3>${t('translationLanguage')}</h3><select id="translation-language">${['English','中文','日本語','한국어','Español','Français','Deutsch'].map(v=>`<option ${data.settings.translation_language===v?'selected':''}>${v}</option>`).join('')}</select></div></div>${permissionsCard()}<div class="notice">${t('keysIndependent')}</div>`;}
function about(){return `<div class="brand" style="padding:10px 0 25px"><img src="assets/logo.svg" alt=""><strong>Vao</strong><span>BETA</span></div><h1>${t('intro')}</h1><p>${t('aboutText')}</p><div class="card"><h2>${t('limits')}</h2><p>${t('limitsText')}</p></div><div class="notice">${t('aboutSource')}</div><p class="muted" style="margin-top:24px">${esc(data.version)} · AGPL-3.0-only · Rust / Tauri / SQLite</p>`;}
function cardConfig(slot){const e=document.querySelector(`[data-slot="${slot}"]`),f=k=>e.querySelector(`[data-field="${k}"]`);return {base:f('base').value.trim(),model:(f('manual').dataset.dirty?f('manual').value:f('model').value).trim(),protocol:slot==='asr'?'openai_audio':f('protocol').value};}
const draftKey=slot=>{const key=document.querySelector(`#${slot}-key`).value;return key?key:null;};
async function refresh(){if(native)data=await ipc('bootstrap');}
async function saveSettings(settings){await ipc('save_settings',{settings});await refresh();theme();}
function action(selector,fn){document.querySelectorAll(selector).forEach(e=>e.addEventListener('click',async ev=>{try{await fn(e,ev);}catch(error){toast(error);}}));}
function bindAutoInputAction(){action('[data-enable-auto-input]',async()=>{await saveSettings({...data.settings,auto_input:true});render();focusResult();toast(t('autoInputEnabled'));});}
function bind(){
 bindAutoInputAction();
 bindPermissions();action('[data-repair-permission]',async e=>{const kind=e.dataset.repairPermission;if(!confirm(t(kind)+'：'+t('permissionRepairConfirm')))return;e.disabled=true;try{data.permissions=await ipc('repair_permission',{kind});await refreshPermissions();}finally{e.disabled=permissionWorkActive();}});action('#reveal-application',async()=>ipc('reveal_application'));action('#refresh-permissions',async()=>refreshPermissions());action('#restart-permissions',async()=>{if(!confirm(t('permissionRestartConfirm')))return;await ipc('restart_for_permissions');});
 action('#setup-auto-input',async()=>{await saveSettings({...data.settings,auto_input:!data.settings.auto_input});render();});
 action('#finish-setup',async()=>{if(!routeReady()||permissionValue('microphone')!=='granted')return;await saveSettings({...data.settings,onboarding_complete:true});render();toast(t('setupDone'));});
 document.querySelector('#trial-consent')?.addEventListener('change',e=>{document.querySelector('#enroll-trial').disabled=!e.target.checked;});
 action('#enroll-trial',async e=>{if(!document.querySelector('#trial-consent')?.checked)return;e.disabled=true;e.textContent=t('enrolling');try{serviceStatus=await ipc('enroll_trial',{consent:true});serviceError='';await refresh();render();toast(t('enrollSuccess'));}finally{if(e.isConnected){e.disabled=!document.querySelector('#trial-consent')?.checked;e.textContent=t('enrollTitle');}}});
 action('#trial-enabled',async()=>{await saveSettings({...data.settings,trial_enabled:data.settings.trial_enabled===false});render();});
 action('#check-trial',async e=>{e.disabled=true;try{await checkTrial();}finally{e.disabled=false;}});
 action('#test-trial',async e=>{e.disabled=true;try{const result=await ipc('test_trial');toast(t('connectionSuccess')+result);}finally{e.disabled=false;await checkTrial();}});
 action('[data-restore-trial]',async e=>{const slot=e.dataset.restoreTrial;if(!confirm(t('trialRestoreConfirm')))return;await saveSettings({...data.settings,[slot]:structuredClone(initial.settings[slot])});render();});

 action('[data-edit-shortcut]',async e=>{await stopCapture();shortcutEditing=e.dataset.editShortcut;shortcutDraft=shortcuts()[shortcutEditing];render();});
 action('#cancel-shortcut',async()=>{await stopCapture();shortcutEditing=null;shortcutDraft=null;render();});
 action('[data-shortcut-preset]',async e=>{await stopCapture();shortcutDraft=e.dataset.shortcutPreset;document.querySelector('#shortcut-input').value=shortcutDraft;});
 document.querySelector('#shortcut-input')?.addEventListener('input',e=>shortcutDraft=e.target.value);
 action('#capture-shortcut',async e=>{await stopCapture();const epoch=++captureEpoch;captureArmed=true;e.textContent=t('capturing');document.querySelector('#shortcut-input').focus();captureTimer=setTimeout(()=>{stopCapture().catch(toast);const p=document.querySelector('#capture-status');if(p)p.textContent=t('captureEnded');},12000);try{const available=await ipc('shortcut_capture',{enabled:true});if(epoch===captureEpoch){document.querySelector('#capture-status').textContent=t(available?'captureReady':'nativeUnavailable');}}catch(err){await stopCapture();throw err;}});
 action('#save-shortcut',async e=>{const slot=shortcutEditing;const value=document.querySelector('#shortcut-input').value.trim();await stopCapture();e.disabled=true;try{const result=await ipc('save_shortcuts',{shortcuts:{...shortcuts(),[slot]:value}});await refresh();shortcutEditing=null;shortcutDraft=null;render();toast(t(result?.active===false?'shortcutPending':'shortcutSaved'));}finally{e.disabled=false;}});
 action('#reset-shortcuts',async e=>{await stopCapture();e.disabled=true;try{await ipc('save_shortcuts',{shortcuts:structuredClone(initial.settings.shortcuts)});await refresh();shortcutEditing=null;shortcutDraft=null;render();toast(t('shortcutSaved'));}finally{e.disabled=false;}});
 document.querySelector('#dictionary')?.addEventListener('input',e=>{document.querySelector('#dictionary-count').textContent=new Set(e.target.value.split('\n').map(s=>s.trim()).filter(Boolean)).size;});
 if(page==='models'&&native){ipc('key_status').then(keys=>{for(const slot of ['asr','llm','backup']){const note=document.querySelector(`[data-connection-note="${slot}"]`);if(note&&keys[slot])note.textContent=t('keyStored');}}).catch(toast);}action('[data-page]',async e=>{clearInterval(demoTimer);demoTimer=null;await stopCapture();shortcutEditing=null;shortcutDraft=null;page=e.dataset.page;await refresh();render();if(page==='guide')drawDemo();});action('#lang',async()=>{await saveSettings({...data.settings,ui_language:data.settings.ui_language==='en'?'zh':'en'});render();if(page==='guide')drawDemo();});action('#theme',async()=>{const opts=['system','light','dark'];await saveSettings({...data.settings,theme:opts[(opts.indexOf(data.settings.theme)+1)%3]});render();if(page==='guide')drawDemo();});
 action('#record',async()=>{await ipc('toggle_recording');await poll();});action('#cancel',async()=>ipc('cancel'));action('#translate',async()=>{await ipc('toggle_translation');await poll();});
 action('[data-copy]',async e=>{await ipc('copy_result',{id:Number(e.dataset.copy)});toast(t('copied'));});
 action('[data-feedback]',async e=>{const id=Number(e.dataset.result),remember=e.dataset.feedback!=='once';const applied=await ipc('feedback',{id,layout:remember?e.dataset.feedback:'paragraph',remember});await refresh();if(snap.result?.id===id&&remember)snap.result.feedback=true;render();toast(t(remember?(applied?'ruleApplied':'feedbackSaved'):'done'));});
 for(const id of ['learning','personalization'])action('#'+id,async()=>{await ipc('profile_action',{action:id,value:!data.profile[id]});await refresh();render();});action('#undo',async()=>{await ipc('profile_action',{action:'undo',value:null});await refresh();render();});action('#forget',async()=>{if(confirm(t('forgetConfirm'))){await ipc('profile_action',{action:'forget',value:null});await refresh();render();}});
 action('#clear-history',async()=>{if(confirm(t('clearConfirm'))){await ipc('clear_history');await refresh();render();}});action('#save-dictionary',async()=>{const dictionary=[...new Set(document.querySelector('#dictionary').value.split('\n').map(s=>s.trim()).filter(Boolean))];await saveSettings({...data.settings,dictionary});toast(t('saved'));});
 action('[data-fetch]',async e=>{const slot=e.dataset.fetch;invalidateConnectionReport(slot);e.disabled=true;try{const list=await ipc('fetch_models',{slot,config:cardConfig(slot),key:draftKey(slot)});const select=document.querySelector(`#${slot}-model`);select.innerHTML=list.map(v=>`<option value="${esc(v)}">${esc(v)}</option>`).join('');const manual=document.querySelector(`#${slot}-manual`);delete manual.dataset.dirty;manual.value=select.value;document.querySelector(`[data-connection-note="${slot}"]`).textContent=list.length+' models';}finally{e.disabled=false;}});
 document.querySelectorAll('[data-field="model"]').forEach(e=>e.addEventListener('change',()=>{const m=e.closest('.model-card').querySelector('[data-field="manual"]');m.value=e.value;delete m.dataset.dirty;}));document.querySelectorAll('[data-field="manual"]').forEach(e=>e.addEventListener('input',()=>e.dataset.dirty='1'));
 action('[data-save-connection]',async e=>{const slot=e.dataset.saveConnection,config=cardConfig(slot),key=draftKey(slot);await saveSettings({...data.settings,[slot]:config});if(key!==null)await ipc('save_key',{slot,value:key,base:config.base});document.querySelector(`#${slot}-key`).value='';toast(t('saved'));});
 action('[data-remove-key]',async e=>{const slot=e.dataset.removeKey;invalidateConnectionReport(slot);await ipc('save_key',{slot,value:'',base:cardConfig(slot).base});toast(t('saved'));});
 action('[data-test]',async e=>{const slot=e.dataset.test,config=cardConfig(slot),epoch=(connectionEpochs[slot]||0)+1;connectionEpochs[slot]=epoch;connectionReports[slot]={status:'testing',config:JSON.stringify(config)};paintConnectionReport(slot);e.disabled=true;try{const result=await ipc('test_llm',{slot,config,key:draftKey(slot)});if(connectionEpochs[slot]===epoch){connectionReports[slot]={status:'success',config:JSON.stringify(config),result};paintConnectionReport(slot);}}catch(error){if(connectionEpochs[slot]===epoch){connectionReports[slot]={status:'failed',config:JSON.stringify(config),error:String(error)};paintConnectionReport(slot);}}finally{e.disabled=false;}});
 document.querySelectorAll('.model-card input,.model-card select').forEach(e=>{for(const event of ['input','change'])e.addEventListener(event,()=>invalidateConnectionReport(e.closest('.model-card').dataset.slot));});
 action('#auto-input',async()=>{await saveSettings({...data.settings,auto_input:!data.settings.auto_input});render();});for(const [id,key] of [['recognition-language','language'],['translation-language','translation_language']])document.querySelector('#'+id)?.addEventListener('change',async e=>{try{await saveSettings({...data.settings,[key]:e.target.value});}catch(err){toast(err);}});
 action('[data-permission]',async e=>ipc('open_permission',{kind:e.dataset.permission}));action('[data-lesson]',async e=>{clearInterval(demoTimer);demoTimer=null;lesson=e.dataset.lesson;demoStep=0;render();drawDemo();});action('#demo-play',async()=>{if(demoTimer){pauseDemo();return;}if(demoStep>=lessonSteps().length-1)demoStep=0;demoTimer=setInterval(()=>{demoStep++;if(demoStep>=lessonSteps().length-1){demoStep=lessonSteps().length-1;pauseDemo();}drawDemo();},2600);drawDemo();});action('#demo-replay',async()=>{pauseDemo();demoStep=0;drawDemo();});action('#demo-prev',async()=>{pauseDemo();demoStep=Math.max(0,demoStep-1);drawDemo();});action('#demo-next',async()=>{pauseDemo();demoStep=Math.min(lessonSteps().length-1,demoStep+1);drawDemo();});action('[data-demo-step]',async e=>{pauseDemo();demoStep=Number(e.dataset.demoStep);drawDemo();});}
function pauseDemo(){clearInterval(demoTimer);demoTimer=null;drawDemo();}
function lessonSteps(){return t(lesson==='selection'?'selectionSteps':lesson==='translate'?'translationSteps':'demoSteps');}
function drawDemo(){
 const e=document.querySelector('#demo');if(!e)return;const en=data.settings.ui_language==='en',steps=lessonSteps(),last=steps.length-1;demoStep=Math.min(demoStep,last);
 const captions={dictate:en?['Click the input field where you want your text.','Press the main shortcut once, then speak naturally.','Press the same shortcut again when you finish speaking.','Vao transcribes and refines. Wait for the result.','With automatic insertion on, text returns to the starting input without sending. Otherwise, preview and copy.']:['先点击要输入文字的位置，让光标出现。','按一次主快捷键，然后自然说出内容。','说完后再按一次相同快捷键，结束录音。','Vao 识别并整理内容，等待结果出现。','自动输入开启时回填起始输入框，不会发送；关闭时先预览再复制。'],selection:en?['Highlight the text you want to ask about or change.','Press the main shortcut and immediately ask or say the change.','Press the main shortcut again to stop.','Vao answers or applies the change without opening its window.','A question stays above the sound bar and is copied. An edit tries to replace the selection.']:['先用鼠标选中文字。','按主快捷键后直接说问题或修改要求。','说完后再按一次主快捷键。','Vao 处理所选文字，不会跳回程序窗口。','问题显示在声浪上方；修改会尝试替换原选区。'],translate:en?['Press the main shortcut to start recording.','This opening stays in the original language.','Press the translation shortcut once. Recording continues.','Only this marked part is translated.','Press the translation shortcut again. Continue in the original language.','Press the main shortcut to finish the whole recording.','Only the marked middle part has been translated.']:['按主快捷键，开始整段录音。','先说需要保持原语言的开头。','按一次翻译快捷键，开启翻译；录音继续。','现在说出需要翻译的那一部分。','再按一次翻译键关闭翻译，继续说原语言。','最后按主快捷键，结束整段录音。','检查结果：只有标记的中间片段被翻译。']};
 const listening=lesson==='translate'?demoStep<5:(demoStep===1);const thinking=lesson==='translate'?demoStep===5:demoStep===3;
 let text='',spoken='',key=shortcutLabel('record');
 if(lesson==='dictate'){spoken=en?'Check the process, then review the error logs.':'检查进程状态，然后核对错误日志。';text=demoStep===last?(en?'1. Check the process.\n2. Review the error logs.':'1. 检查进程状态。\n2. 核对错误日志。'):'';}
 if(lesson==='selection'){spoken=en?'Make this more polite.':'帮我改得更礼貌一点。';text=demoStep===last?(en?'Could you please send me the report tomorrow?':'请你明天把报告发给我，好吗？'):(en?'Send me the report tomorrow.':'明天把报告发给我。');}
 if(lesson==='translate'){key=(demoStep===2||demoStep===4)?shortcutLabel('translate'):shortcutLabel('record');spoken=demoStep===3?(en?'See you tomorrow at four.':'明天下午四点见。'):demoStep>=4?(en?'Keep the ending in English.':'结尾也保持中文。'):(en?'Keep the opening in English.':'开头保持中文。');text=demoStep===last?(en?'Keep the opening in English.\n明天下午四点见。\nKeep the ending in English.':'开头保持中文。\nSee you tomorrow at 4 p.m.\n结尾也保持中文。'):'';}
 if(isWindows()&&lesson==='dictate'&&demoStep===last)captions.dictate[last]=en?'With automatic insertion on, Vao tries the starting native text field (such as Notepad). Unsupported or changed fields keep the result for copying. It never sends.':'自动输入开启时尝试回填起始原生文本框（如记事本）；不支持或已变化的输入框保留结果供复制，不会发送。';
 const selection=lesson==='selection'&&demoStep!==last;
 e.innerHTML=`<div class="demo-app"><div class="demo-title"><i></i><i></i><i></i><span>${t('demoWindow')}</span></div><div class="demo-field">${selection?`<mark>${esc(text)}</mark>`:text?esc(text):'<span class="demo-caret"></span>'}</div></div><div class="demo-instruction"><kbd>${esc(key)}</kbd><p>${captions[lesson][demoStep]}</p></div>${(listening&&demoStep>0)||thinking?`<div class="voicebar ${thinking?'thinking':'demo-listening'}"><span>${t(thinking?'thinking':'listening')}</span>${wave()}</div>`:''}${listening&&demoStep>0?`<p class="demo-spoken">${t('demoSpeaking')}：${esc(spoken)}</p>`:''}${lesson==='translate'&&demoStep>=2&&demoStep<=4?`<span class="badge">${en?(demoStep<4?'Translation ON · recording continues':'Translation OFF · recording continues'):(demoStep<4?'翻译已开启 · 仍在录音':'翻译已关闭 · 仍在录音')}</span>`:''}`;
 document.querySelectorAll('[data-demo-step]').forEach(button=>{const selected=Number(button.dataset.demoStep)===demoStep;button.classList.toggle('active',selected);button.setAttribute('aria-current',selected?'step':'false');});
 document.querySelector('#demo-progress').textContent=(demoStep+1)+' / '+steps.length;
 document.querySelector('#demo-play').textContent=t(demoTimer?'pause':'play');document.querySelector('#demo-prev').disabled=demoStep===0;document.querySelector('#demo-next').disabled=demoStep===last;
}
function noticeText(code){const en=data.settings.ui_language==='en';const text={
 no_audio:['没有录到声音，请重新试一次','No audio captured. Please try again.'],
 too_short:['录音太短，请说完后再结束','Recording too short. Finish speaking first.'],
 no_speech:['没有识别到内容，请重新试一次','No speech recognized. Please try again.'],
 no_microphone:['未连接麦克风，请连接后重试','No microphone connected. Connect one and retry.'],
 microphone_unavailable:['麦克风不可用，请检查连接后重试','Microphone unavailable. Check the connection.'],
 selection_unreadable:['无法读取所选文字，请重新选择后再试','Could not read the selection. Select it again.'],
 delivery_target_unavailable:['无法返回原输入位置，结果已保存在历史中','Original input unavailable. Result saved in History.'],
 selection_ready:['已选中文字，请直接说问题或修改要求','Text selected. Ask a question or say how to change it.']};return text[code]?.[en?1:0]||'';}
let barFitFrame=0,lastBarHeight=0;
function fitWindowsBar(){
 if(!native||!barMode||!isWindows()||barFitFrame)return;
 barFitFrame=requestAnimationFrame(()=>{
  barFitFrame=0;
  const notice=document.querySelector('#bar-notice'),bar=document.querySelector('.bar-stage > .voicebar');
  let height=20;
  if(notice&&!notice.classList.contains('hidden')){
   height+=notice.classList.contains('answer')?document.querySelector('.answer-toolbar').offsetHeight+document.querySelector('#bar-delivery-hint').offsetHeight+document.querySelector('#bar-message').scrollHeight+2:notice.scrollHeight+2;
  }
  if(bar&&!bar.classList.contains('hidden')&&!document.querySelector('.notice-only'))height+=bar.offsetHeight+(height>20?10:0);
  height=Math.ceil(Math.max(74,Math.min(360,height)));
  if(height!==lastBarHeight){lastBarHeight=height;ipc('fit_voicebar',{height}).catch(()=>{lastBarHeight=0;});}
 });
}
window.addEventListener('resize',()=>{if(barMode&&isWindows())fitWindowsBar();});
function updateLive(){const phase=snap.session.phase,active=['preparing','listening','processing'].includes(phase),listen=phase==='listening';if(barMode){const notice=document.querySelector('#bar-notice'),bar=document.querySelector('.voicebar');const answer=snap.answer||'';const code=answer?'':(snap.notice||(listen&&snap.selection?'selection_ready':null));if(notice){const hint=document.querySelector('#bar-delivery-hint');if(hint){hint.classList.toggle('hidden',!isWindows()||!answer||!snap.error);hint.textContent=data.settings.ui_language==='en'?'Original input unavailable. Check it before copying this result.':'原输入位置未确认，请检查原框后复制结果。';}const copy=document.querySelector('#bar-copy');if(copy){copy.disabled=!snap.result;copy.textContent=t(copy.dataset.copied===String(snap.session.id)+':'+String(snap.result?.id||0)?'copied':'copy');}document.querySelector('#bar-message').textContent=answer?((isWindows()?'':(data.settings.ui_language==='en'?'Copied.\n\n':'已复制。\n\n'))+answer):noticeText(code);document.querySelector('.answer-toolbar')?.classList.toggle('hidden',!answer);notice.classList.toggle('hidden',!answer&&!code);notice.classList.toggle('answer',!!answer);}document.querySelector('.bar-stage')?.classList.toggle('notice-only',!answer&&['no_microphone','microphone_unavailable','selection_unreadable','delivery_target_unavailable'].includes(code));if(bar){bar.classList.toggle('thinking',!listen&&!snap.notice&&!answer);bar.classList.toggle('stopped',!!snap.notice&&!answer);bar.classList.toggle('hidden',!!answer);}const label=document.querySelector('#bar-label');if(label)label.textContent=answer?(isWindows()?(data.settings.ui_language==='en'?'Result':'处理结果'):(data.settings.ui_language==='en'?'Answer copied':'追问结果 · 已复制')):snap.notice?(data.settings.ui_language==='en'?'Stopped':'已停止'):(listen&&snap.translating?t('translationActive')+' · '+data.settings.translation_language:t(listen?'listening':'thinking'));fitWindowsBar();}else{document.querySelector('#live')?.classList.toggle('hidden',!active);const label=document.querySelector('#live-label');if(label)label.textContent=listen&&snap.translating?t('translationActive')+' · '+data.settings.translation_language:t(listen?'listening':'thinking');document.querySelector('#live .voicebar')?.classList.toggle('thinking',!listen);const record=document.querySelector('#record');if(record){record.textContent=t(listen?'stop':'start');record.disabled=active&&!listen;}const trans=document.querySelector('#translate');if(trans){trans.disabled=active&&!listen;trans.classList.toggle('primary',snap.translating);}const status=document.querySelector('#top-status');if(status){status.textContent=active?t(listen?'listening':'thinking'):t(readiness());document.querySelector('.status-dot')?.classList.toggle('pending',!active&&!['setupReady','setupWindowsPreview'].includes(readiness()));}}document.querySelectorAll('#live .wave,.bar-mode .wave').forEach(w=>w.querySelectorAll('i').forEach((e,i)=>e.style.height=(4+(snap.level||0)*70*(.4+.6*Math.abs(Math.sin(i*1.7+Date.now()/160))))+'px'));}
function focusResult(){requestAnimationFrame(()=>{document.querySelector('#result-card')?.scrollIntoView({block:'start'});});}
async function poll(){
 if(!native||polling)return;polling=true;
 try{
  snap=await ipc('snapshot');
  const signature=snap.result?`${snap.session.id}:${snap.result.id}:${snap.result.text}`:'';
  const changed=signature!==lastResult,phaseChanged=lastPhase!==snap.session.phase;
  if(changed&&signature)await refresh();
  const terminal=['ready','failed'].includes(snap.session.phase);
  if(!barMode&&!snap.notice&&terminal&&(phaseChanged||changed)){
   page='home';render();focusResult();
  }else if(!barMode&&page==='home'&&changed){
   if(!signature||!lastResult)render();else{document.querySelector('#result-content').innerHTML=resultHtml(snap.result);bindResult();}
  }
  lastResult=signature;lastPhase=snap.session.phase;
  if(barMode&&snap.translationLanguage)data.settings.translation_language=snap.translationLanguage;
  if(barMode&&snap.uiLanguage&&(data.settings.ui_language!==snap.uiLanguage||data.settings.theme!==snap.theme)){data.settings.ui_language=snap.uiLanguage;data.settings.theme=snap.theme;render();}
  if(snap.error&&snap.error!==lastError){lastError=snap.error;if(!barMode&&!snap.notice&&snap.session.phase!=='failed')toast(snap.error);}
  if(!snap.error)lastError='';updateLive();
 }catch(e){if(!barMode&&lastError!==String(e)){lastError=String(e);toast(e);}}finally{polling=false;}
}

function bindResult(){bindAutoInputAction();action('[data-copy]',async e=>{await ipc('copy_result',{id:Number(e.dataset.copy)});toast(t('copied'));});action('[data-feedback]',async e=>{const remember=e.dataset.feedback!=='once';const applied=await ipc('feedback',{id:Number(e.dataset.result),layout:remember?e.dataset.feedback:'paragraph',remember});await refresh();if(remember)snap.result.feedback=true;render();toast(t(remember?(applied?'ruleApplied':'feedbackSaved'):'done'));});}
async function nativeRegressionCheck(){
 const original=structuredClone(shortcuts());const checks=[];let failure=null;
 const check=(name,ok)=>{checks.push({name,passed:!!ok});ipc('report_frontend_state',{stage:JSON.stringify({kind:'native-progress',name,passed:!!ok})}).catch(()=>{});if(!ok)throw Error(name);};
 try{
  await ipc('report_frontend_state',{stage:'native-start'});
  for(const [command,args] of [['repair_permission',{kind:'input_monitoring'}],['restart_for_permissions',{}]]){let blocked=false;try{await ipc(command,args);}catch(e){blocked=String(e).includes('isolated tests');}check(command+'_blocked_in_isolated_tests',blocked);}
  check('permission_identity_is_public',data.permissions?.bundle==='app.vaovao.desktop');check('permission_status_is_explicit',['granted','denied','restricted','not_determined','unknown'].includes(data.permissions?.microphone));
  page='models';render();await Promise.race([new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r))),new Promise(r=>setTimeout(r,800))]);
  const el=document.querySelector('.scroll');check('wkwebview_bounded_scroller',el.clientHeight>0&&el.clientHeight<innerHeight&&el.scrollHeight>el.clientHeight);
  el.scrollTop=el.scrollHeight;check('wkwebview_bottom_reachable',el.scrollTop>0&&Math.abs(el.scrollTop+el.clientHeight-el.scrollHeight)<2);
  check('wkwebview_toolbar_fixed',document.querySelector('.toolbar').getBoundingClientRect().top===0);
  page='settings';shortcutEditing='record';shortcutDraft='';render();
  for(const chord of ['Fn','Fn+Space','Super+KeyC','Shift+Super+Slash','Super+ArrowLeft']){
   document.querySelector('#shortcut-input').value='';document.querySelector('#capture-shortcut').click();
   for(let n=0;n<40&&!document.querySelector('#capture-status').textContent.includes(t('captureReady'));n++)await new Promise(r=>setTimeout(r,50));
   check('local_capture_armed_'+chord,captureArmed);
   await ipc('probe_capture',{chord});
   for(let n=0;n<40&&document.querySelector('#shortcut-input').value!==chord;n++)await new Promise(r=>setTimeout(r,50));
   check('native_window_capture_'+chord,document.querySelector('#shortcut-input').value===chord&&!captureArmed);
  }
  await stopCapture();shortcutEditing=null;shortcutDraft=null;
  const displays=await ipc('probe_voicebar');check('voicebar_inside_each_connected_display',displays.length>0&&displays.every(d=>d.contained&&d.screen_matches&&d.size_matches&&d.position_restored!==false));await ipc('report_frontend_state',{stage:JSON.stringify({kind:'voicebar-display-checks',displays})});
  await ipc('save_shortcuts',{shortcuts:{record:'Fn',translate:'Fn+Space',cancel:'Escape'}});
  if(data.permissions?.accessibility==='granted')for(const cmd of ['probe_selection','probe_input_feedback','probe_composer','probe_overlay','probe_locked_target']){const result=await ipc(cmd);await ipc('report_frontend_state',{stage:JSON.stringify({kind:cmd,...result})});for(const item of result.checks)check(item.name,item.passed);}
  if(data.permissions?.accessibility!=='granted')await ipc('report_frontend_state',{stage:JSON.stringify({kind:'skipped-native-checks',reason:'Accessibility not granted for public bundle',checks:['probe_selection','probe_input_feedback','probe_composer']})});
  const probe={record:'Control+Alt+Shift+F19',translate:'Control+Alt+Shift+F20',cancel:'Escape'};
  await ipc('save_shortcuts',{shortcuts:probe});await refresh();
  check('native_settings_saved',data.settings.shortcuts.record===probe.record&&data.settings.shortcuts.translate===probe.translate);
  const runtime=await ipc('shortcut_status');check('native_keys_registered',runtime.record.toLowerCase().includes('f19')&&runtime.translate.toLowerCase().includes('f20')&&runtime.registered.length===2);
  let rejected=false;try{await ipc('save_shortcuts',{shortcuts:{...probe,translate:probe.record}});}catch(_){rejected=true;}
  check('native_duplicate_rejected',rejected);await refresh();check('rejected_change_preserves_saved_settings',data.settings.shortcuts.translate===probe.translate);
  let fnAvailable=false,fnError=null;
  let fnSaved=false;try{const result=await ipc('save_shortcuts',{shortcuts:{record:'Fn',translate:'Fn+Space',cancel:'Escape'}});fnAvailable=result.active;fnSaved=result.saved;}catch(e){fnError=String(e);}
  await refresh();const fnRuntime=await ipc('shortcut_status');
  check('fn_saved_and_activation_is_honest',fnAvailable?(fnRuntime.native_ready&&fnRuntime.record==='Fn'&&fnRuntime.translate==='Fn+Space'):(fnSaved&&!fnRuntime.native_ready&&data.settings.shortcuts.record==='Fn'));
  await ipc('report_frontend_state',{stage:JSON.stringify({kind:'fn-native-capability',available:fnAvailable,permissionRequired:!fnAvailable})});

 }catch(e){failure=String(e);}finally{
  try{await ipc('save_shortcuts',{shortcuts:original});await refresh();check('original_shortcuts_restored',Object.keys(original).every(k=>data.settings.shortcuts[k]===original[k]));}catch(e){failure=(failure||'')+' restore: '+String(e);}
  page='home';render();await ipc('report_frontend_state',{stage:JSON.stringify({kind:'native-checks',checks,error:failure})});
 }
}
async function init(){if(native)ipc('report_frontend_state',{stage:'init-start'}).catch(()=>{});try{await refresh();if(native)ipc('report_frontend_state',{stage:'bootstrap-done'}).catch(()=>{});snap=native?await ipc('snapshot'):snap;}catch(e){toast(e);}render();if(native){await window.__TAURI__.event.listen('shortcut-captured',event=>acceptCaptured(event.payload));await window.__TAURI__.event.listen('shortcut-capture-progress',event=>showCaptureProgress(event.payload));const verify=await ipc('report_ui_ready');if(verify&&!barMode)await nativeRegressionCheck();window.__TAURI__.event.listen('result-ready',()=>poll());window.__TAURI__.event.listen('session-updated',()=>poll());}if(!native)toast(t('browser'));async function tick(){await poll();setTimeout(tick,['preparing','listening','processing'].includes(snap.session.phase)?(barMode?90:180):900);}tick();}
window.addEventListener('focus',()=>{if(!barMode)refreshPermissions().catch(()=>{});});
setInterval(()=>{if(page==='settings'&&!barMode&&!document.hidden)refreshPermissions().catch(()=>{});},5000);
window.addEventListener('blur',()=>{if(captureArmed)stopCapture().catch(()=>{});});
init();

})();
