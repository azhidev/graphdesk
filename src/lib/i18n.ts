/** English source keys are centralized and checked at call sites. Notes are never translated. */
export type Language = 'en' | 'fa';
export const messages = {
  'Open graph…': 'باز کردن گراف…', 'Create graph…': 'ساخت گراف…',
  'Remember recent graphs': 'به‌خاطر سپردن گراف‌های اخیر', 'Reopen last graph on startup': 'باز کردن آخرین گراف هنگام شروع',
  'Remembering stores file paths privately on this computer, not notes. Turning it off clears remembered paths.': 'با فعال کردن این گزینه، فقط مسیر فایل‌ها به‌صورت خصوصی در این رایانه ذخیره می‌شود، نه یادداشت‌ها. غیرفعال کردن آن مسیرهای ذخیره‌شده را پاک می‌کند.',
  'Recent graphs': 'گراف‌های اخیر', 'Reopen last graph': 'باز کردن آخرین گراف', 'Forget graph': 'فراموش کردن گراف',
  'Clear remembered graphs': 'پاک کردن گراف‌های به‌خاطر سپرده‌شده', 'No remembered graphs. Open a graph to add it here.': 'گرافی به‌خاطر سپرده نشده است. برای افزودن، گرافی باز کنید.',
  'Graph registry': 'فهرست گراف‌ها', 'Remembered graphs could not be loaded. Paths will not be remembered until you explicitly reset these settings.': 'فهرست گراف‌ها خوانده نشد. تا بازنشانی صریح این تنظیمات، مسیری ذخیره نمی‌شود.',
  'Graph opened, but could not be remembered.': 'گراف باز شد، اما در فهرست ذخیره نشد.', 'Graph registry changes could not be saved.': 'تغییرات فهرست گراف‌ها ذخیره نشد.',
  'Reset remembered paths (turn off)': 'بازنشانی مسیرهای ذخیره‌شده (غیرفعال کردن)',
  'Opening graph…': 'در حال باز کردن گراف…', 'Choosing graph…': 'در حال انتخاب گراف…',
  'Graph opened, but today could not be loaded. Retry Today or switch graphs.': 'گراف باز شد، اما روزانهٔ امروز بارگذاری نشد. دوباره تلاش کنید یا گراف را تغییر دهید.',
  'Retry Today': 'تلاش دوباره برای امروز', 'Back': 'بازگشت', 'Forward': 'جلو', 'Navigation history': 'تاریخچهٔ پیمایش',
  'Go to page': 'رفتن به صفحه', 'Filter page titles': 'فیلتر عنوان صفحه‌ها',
  '↑/↓ choose · Enter opens · Esc cancels · Up to 100 pages': '↑/↓ انتخاب · Enter باز کردن · Esc انصراف · حداکثر ۱۰۰ صفحه',
  'No matching pages.': 'صفحه‌ای مطابق جست‌وجو نیست.', 'This destination is no longer available. Choose another page.': 'این مقصد دیگر در دسترس نیست. صفحهٔ دیگری انتخاب کنید.',
  'Editor input changed during graph opening. Try again after saving.': 'هنگام باز کردن گراف، متن ویرایشگر تغییر کرد. پس از ذخیره دوباره تلاش کنید.',
  'The graph operation changed. Try again.': 'عملیات گراف تغییر کرد. دوباره تلاش کنید.',
  'graph switch state unavailable': 'وضعیت تغییر گراف در دسترس نیست. دوباره تلاش کنید.',
  'graph session changed; prepare again': 'نشست گراف تغییر کرد. دوباره گراف را باز کنید.',
  'graph preparation expired; prepare again': 'آماده‌سازی گراف منقضی شد. دوباره تلاش کنید.',
  'remembered path now contains a different graph': 'اکنون گراف دیگری در مسیر ذخیره‌شده قرار دارد. فایل درست را صریحاً انتخاب کنید.',
  'graph identity changed while opening': 'شناسهٔ گراف هنگام باز کردن تغییر کرد. فایل درست را دوباره انتخاب کنید.',
  'automatic opening requires a remembered graph identity': 'باز کردن خودکار به شناسهٔ گراف ذخیره‌شده نیاز دارد.',
  'creation cannot use a remembered identity': 'ساخت گراف نمی‌تواند از شناسهٔ ذخیره‌شده استفاده کند.',
  'graph path is not representable as Unicode': 'مسیر گراف با متن یونیکد قابل نمایش نیست.',
  'graph path is empty': 'مسیر گراف خالی است.', 'graph filename required': 'نام فایل گراف لازم است.',
  'selected path is not representable as Unicode': 'مسیر انتخاب‌شده با متن یونیکد قابل نمایش نیست.',
  'native dialog did not return a filesystem path': 'انتخابگر بومی مسیر فایل برنگرداند. از ورود دستی مسیر استفاده کنید.',
  'Graph file unavailable. Choose its current location; no replacement was created.': 'فایل گراف در دسترس نیست. مسیر فعلی آن را انتخاب کنید؛ فایل جایگزین ساخته نشده است.',
  'This graph needs an explicit open. Automatic reopening never upgrades old or unsupported graph formats.': 'این گراف باید صریحاً باز شود. باز کردن خودکار، قالب قدیمی یا پشتیبانی‌نشده را ارتقا نمی‌دهد.',
  'Finish or cancel the first-note form before switching graphs.': 'پیش از تغییر گراف، فرم نخستین یادداشت را کامل کنید یا انصراف دهید.',
  'Keyboard shortcuts': 'میان‌برهای صفحه‌کلید',
  'Ctrl/Cmd+K: search · Ctrl/Cmd+P: pages · Ctrl/Cmd+O: open graph · Ctrl/Cmd+Shift+J: capture · Alt+←/→: history outside text fields': 'Ctrl/Cmd+K: جست‌وجو · Ctrl/Cmd+P: صفحه‌ها · Ctrl/Cmd+O: باز کردن گراف · Ctrl/Cmd+Shift+J: یادداشت · Alt+←/→: تاریخچه بیرون از فیلدهای متن',
  'Today': 'امروز', 'Tasks': 'کارها', 'Capture today': 'یادداشت امروز',
  'Previous day': 'روز قبل', 'Next day': 'روز بعد', 'Journal navigation': 'پیمایش روزانه',
  'Settings': 'تنظیمات', 'Language': 'زبان', 'Calendar': 'گاه‌شماری',
  'Gregorian': 'میلادی', 'Jalali': 'جلالی', 'Search mode': 'حالت جست‌وجو',
  'Original phrase search': 'جست‌وجوی عبارتی اصلی', 'Persian-friendly': 'سازگار با فارسی',
  'Preferences could not be loaded. Safe defaults are in use.': 'تنظیمات خوانده نشد. پیش‌فرض‌های امن فعال هستند.',
  'Preferences could not be saved. Your previous settings remain active.': 'تنظیمات ذخیره نشد. تنظیمات قبلی همچنان فعال هستند.',
  'Switch graph': 'تغییر گراف', 'Cancel graph switch': 'انصراف از تغییر گراف', 'Graph:': 'گراف:',
  'Open a local graph to begin your daily workspace.': 'برای شروع، یک گراف محلی باز کنید.',
  'Graph selection': 'انتخاب گراف', 'Database path (on this computer)': 'مسیر پایگاه داده در این رایانه',
  'Full path to notes.sqlite3': 'مسیر کامل فایل notes.sqlite3', 'Open graph': 'باز کردن گراف', 'Create graph': 'ساخت گراف',
  'The parent directory must exist. Create never overwrites an existing file.': 'پوشهٔ مقصد باید موجود باشد. ساخت گراف هیچ فایل موجودی را بازنویسی نمی‌کند.',
  'Search pages and blocks': 'جست‌وجوی صفحه‌ها و بلوک‌ها', 'Search · Ctrl/Cmd+K': 'جست‌وجو · Ctrl/Cmd+K',
  'Phrase search · ↑/↓ choose · Enter opens · Esc clears': 'جست‌وجوی عبارتی · ↑/↓ انتخاب · Enter باز کردن · Esc پاک کردن',
  'Searching…': 'در حال جست‌وجو…', 'No matches.': 'نتیجه‌ای پیدا نشد.', 'Page': 'صفحه', 'Block': 'بلوک',
  'Search results': 'نتایج جست‌وجو', 'Up to 100 results per kind. Refine your phrase to find more.': 'حداکثر ۱۰۰ نتیجه از هر نوع. برای نتایج دیگر عبارت دقیق‌تری بنویسید.',
  'Journal date': 'تاریخ روزانه', 'Year': 'سال', 'Month': 'ماه', 'Day': 'روز', 'Open journal': 'باز کردن روزانه',
  'Enter a valid date.': 'تاریخ معتبری وارد کنید.', 'This date cannot be used in Jalali. Its Gregorian identity is unchanged.': 'این تاریخ در گاه‌شماری جلالی قابل استفاده نیست. تاریخ میلادی آن تغییری نکرده است.',
  'Pages': 'صفحه‌ها', 'New page': 'صفحهٔ جدید', 'New page title': 'عنوان صفحهٔ جدید', 'Create page': 'ساخت صفحه',
  'Page content': 'محتوای صفحه', 'Rename page': 'تغییر نام صفحه', 'Page title': 'عنوان صفحه',
  'Incoming backlinks': 'پیوندهای ورودی', 'Backlinks': 'پیوندهای ورودی', 'No incoming links.': 'پیوند ورودی وجود ندارد.',
  'No notes for this day yet. Browsing dates does not create a journal.': 'این روز هنوز یادداشتی ندارد. مرور تاریخ‌ها روزانه‌ای ایجاد نمی‌کند.',
  'Add a note': 'افزودن یادداشت', 'Select a page, open a journal, or capture a note for today.': 'صفحه‌ای انتخاب کنید، روزانه‌ای باز کنید یا یادداشتی برای امروز بنویسید.',
  'Workspace': 'فضای کار', 'Plain-text block editor · Undo and collapse state are session-local. Acknowledged saves survive restart.': 'ویرایشگر بلوکی متن ساده · واگرد و حالت جمع‌شدن موقتی هستند. ذخیره‌های تأییدشده پس از بازگشایی باقی می‌مانند.',
  'Review local recovery drafts': 'بررسی پیش‌نویس‌های بازیابی محلی',
  'Recovery drafts found for {count} graph(s). Open the original graph to compare and recover text.': 'پیش‌نویس بازیابی برای {count} گراف یافت شد. گراف اصلی را برای مقایسه و بازیابی باز کنید.',
  'First note for {date}': 'نخستین یادداشت برای {date}', 'Note text': 'متن یادداشت', 'Save first note': 'ذخیرهٔ نخستین یادداشت',
  'Finalized text is captured privately before submission. The journal is created only when you submit.': 'متن نهایی پیش از ارسال به‌صورت خصوصی ثبت می‌شود. روزانه فقط هنگام ارسال ساخته می‌شود.',
  'Cancel': 'انصراف', 'Discard form text and cancel': 'حذف متن فرم و انصراف',
  'Loading…': 'در حال بارگذاری…', 'Opening journal…': 'در حال باز کردن روزانه…', 'Preparing capture…': 'در حال آماده‌سازی یادداشت…', 'Saving first note…': 'در حال ذخیرهٔ نخستین یادداشت…',
  'Saved': 'ذخیره شد', 'Saving…': 'در حال ذخیره…', 'Unsaved': 'ذخیره نشده',
  'Composing — finish input before leaving': 'ورودی در حال ترکیب است؛ پیش از خروج آن را تمام کنید',
  'Error — save failed': 'خطا — ذخیره نشد', 'Error — unsaved changes may remain': 'خطا — ممکن است تغییرات ذخیره‌نشده باقی مانده باشند',
  'Recovery pending': 'ثبت بازیابی در انتظار', 'Recovery stored': 'بازیابی محلی ثبت شد', 'Recovery failed': 'ثبت بازیابی ناموفق بود',
  'Retry recovery snapshot': 'تلاش دوباره برای ثبت بازیابی',
  'Block editor actions': 'ابزارهای ویرایش بلوک', 'Undo': 'واگرد', 'Redo': 'ازنو', 'Indent': 'افزایش تورفتگی', 'Outdent': 'کاهش تورفتگی',
  'Move up': 'انتقال به بالا', 'Move down': 'انتقال به پایین', 'Delete…': 'حذف…', 'Delete block and descendants': 'حذف بلوک و زیرشاخه‌ها',
  'Text save conflict': 'تعارض ذخیرهٔ متن', 'Persisted text changed. Replacing it requires your explicit choice.': 'متن ذخیره‌شده تغییر کرده است. جایگزینی آن به انتخاب صریح شما نیاز دارد.',
  'Persisted version': 'نسخهٔ ذخیره‌شده', 'Your unsaved version': 'نسخهٔ ذخیره‌نشدهٔ شما',
  'Replace compared persisted text with my draft': 'جایگزینی متن مقایسه‌شده با پیش‌نویس من',
  'Delete {count} block(s)?': 'حذف {count} بلوک؟', 'Delete {count} block(s)': 'حذف {count} بلوک',
  'The selected block and all descendants will be deleted. You can undo this during this graph session.': 'بلوک انتخاب‌شده و همهٔ زیرشاخه‌ها حذف می‌شوند. در همین نشست گراف می‌توانید واگرد کنید.',
  'Retry saves / clear error': 'تلاش دوباره برای ذخیره / پاک کردن خطا', 'Discard unsaved drafts and reload': 'حذف پیش‌نویس‌های ذخیره‌نشده و بارگذاری دوباره',
  'Enter: split · Shift+Enter: newline · Tab / Shift+Tab: nest · Alt+Shift+↑/↓: reorder · Escape: leave text editor. Changes autosave.': 'Enter: تقسیم · Shift+Enter: خط جدید · Tab / Shift+Tab: تورفتگی · Alt+Shift+↑/↓: جابه‌جایی · Escape: خروج از متن. تغییرات خودکار ذخیره می‌شوند.',
  'Add first block': 'افزودن نخستین بلوک', 'Page blocks': 'بلوک‌های صفحه',
  'Open referenced page': 'باز کردن صفحهٔ پیوندشده', 'Create referenced page': 'ساخت صفحهٔ پیوندشده', '(create)': '(ساخت)',
  'Task status': 'وضعیت کار', 'No task': 'بدون کار', 'TODO': 'برای انجام', 'DOING': 'در حال انجام', 'DONE': 'انجام شده', 'CANCELLED': 'لغو شده',
  'Expand children': 'باز کردن زیرشاخه‌ها', 'Collapse children': 'جمع کردن زیرشاخه‌ها', 'Block text': 'متن بلوک', 'Empty block — click to edit': 'بلوک خالی — برای ویرایش انتخاب کنید', 'Page suggestions': 'پیشنهادهای صفحه',
  'Tasks from this graph. Open a task to edit it on its original page.': 'کارهای این گراف. برای ویرایش هر کار، صفحهٔ اصلی آن را باز کنید.',
  'Status': 'وضعیت', 'Active (TODO + DOING)': 'فعال (برای انجام + در حال انجام)', 'All pages and journals': 'همهٔ صفحه‌ها و روزانه‌ها',
  'Refresh tasks': 'تازه‌سازی کارها', 'Loading tasks…': 'در حال بارگذاری کارها…', 'Use Refresh tasks to retry.': 'برای تلاش دوباره، کارها را تازه‌سازی کنید.',
  'No tasks match these filters. Set a block to TODO or DOING in a page or journal.': 'کاری با این فیلترها یافت نشد. وضعیت بلوکی را به «برای انجام» یا «در حال انجام» تغییر دهید.',
  'Empty task': 'کار خالی', 'Load more tasks': 'کارهای بیشتر',
  'Recoverable text drafts': 'پیش‌نویس‌های قابل بازیابی',
  'Stored only on this computer, outside your graph. Unresolved snapshots expire after 14 days. Compare before recovering; no structure is replayed.': 'فقط در این رایانه و خارج از گراف نگهداری می‌شود. نسخه‌های حل‌نشده پس از ۱۴ روز منقضی می‌شوند. پیش از بازیابی مقایسه کنید؛ ساختار بازپخش نمی‌شود.',
  'Compare / Recover': 'مقایسه / بازیابی', 'Discard recovery draft…': 'حذف پیش‌نویس بازیابی…', 'Compare recovery draft': 'مقایسهٔ پیش‌نویس بازیابی',
  'Original block unavailable': 'بلوک اصلی در دسترس نیست', 'Recovery version — select to copy': 'نسخهٔ بازیابی — برای کپی انتخاب کنید',
  'Recover replaces only the displayed block text, if its revision has not changed. Both versions remain available until you resolve this choice.': 'بازیابی فقط متن نمایش‌داده‌شدهٔ بلوک را در صورت تغییر نکردن نسخه جایگزین می‌کند. تا تصمیم شما هر دو نسخه باقی می‌مانند.',
  'Recover this text': 'بازیابی این متن', 'Close comparison': 'بستن مقایسه', 'Discard this recovery draft?': 'این پیش‌نویس بازیابی حذف شود؟',
  'This permanently removes the selected recovery copy, not the persisted block. It cannot be undone.': 'نسخهٔ بازیابی انتخاب‌شده برای همیشه حذف می‌شود، نه بلوک ذخیره‌شده. این کار قابل واگرد نیست.',
  'Discard recovery draft': 'حذف پیش‌نویس بازیابی', 'Pending journal notes': 'یادداشت‌های روزانهٔ در انتظار', 'Restore first-note form': 'بازگردانی به فرم نخستین یادداشت',
  'Restore never submits automatically. Compare persisted content before submitting again.': 'بازگردانی خودکار ارسال نمی‌کند. پیش از ارسال دوباره، محتوای ذخیره‌شده را مقایسه کنید.',
  'No committed note exists.': 'یادداشت ثبت‌شده‌ای وجود ندارد.', 'This note was deleted after submission. It will not be recreated.': 'این یادداشت پس از ارسال حذف شده است و دوباره ساخته نمی‌شود.',
  'The persisted note differs. Copy the retained text if needed; it will not be overwritten.': 'یادداشت ذخیره‌شده متفاوت است. در صورت نیاز متن نگه‌داشته‌شده را کپی کنید؛ متن ذخیره‌شده بازنویسی نمی‌شود.',
  'Technical details': 'جزئیات فنی', 'Operation failed. Your text has not been discarded.': 'عملیات ناموفق بود. متن شما دور ریخته نشده است.',
  'Finish text composition before leaving the editor': 'پیش از خروج از ویرایشگر، ترکیب ورودی را تمام کنید',
  'Wait for the current operation': 'منتظر پایان عملیات جاری بمانید',
  'Recovery storage failed — unsaved text is not protected. Keep the app open and retry recovery.': 'ذخیرهٔ بازیابی ناموفق بود؛ متن ذخیره‌نشده محافظت نشده است. برنامه را باز نگه دارید و دوباره تلاش کنید.',
  'Recovery storage unavailable; unsaved text is not protected. Keep the app open and retry.': 'ذخیرهٔ بازیابی در دسترس نیست؛ برنامه را باز نگه دارید و دوباره تلاش کنید.',
  'Finish or cancel the first-note form before closing.': 'پیش از بستن، فرم نخستین یادداشت را کامل کنید یا انصراف دهید.',
  'Text save acknowledgement timed out. Draft retained; Retry compares persisted state.': 'تأیید ذخیره به‌موقع نرسید. پیش‌نویس نگه داشته شد؛ تلاش دوباره وضعیت ذخیره‌شده را مقایسه می‌کند.',
  'Recovery inventory unavailable. Open the original graph and retry recovery inspection.': 'فهرست بازیابی در دسترس نیست. گراف اصلی را باز کنید و دوباره بررسی کنید.',
  'Matching page no longer exists; reopen the graph.': 'صفحهٔ نتیجه دیگر وجود ندارد؛ گراف را دوباره باز کنید.',
  'Referenced page no longer exists; reopen the graph.': 'صفحهٔ پیوندشده دیگر وجود ندارد؛ گراف را دوباره باز کنید.',
  'Wait for the current operation before closing.': 'پیش از بستن، منتظر پایان عملیات جاری بمانید.',
  'A page already uses this date title. Rename that page before creating this journal.': 'صفحه‌ای با این عنوان تاریخ وجود دارد. پیش از ساخت روزانه، نام آن صفحه را تغییر دهید.',
  'Database is busy. Wait for other writes to finish, then retry.': 'پایگاه داده مشغول است. منتظر پایان نوشتن‌های دیگر بمانید و دوباره تلاش کنید.',
  'Could not save. Check available space and permissions; keep the app open.': 'ذخیره انجام نشد. فضای آزاد و مجوزهای دسترسی را بررسی کنید و برنامه را باز نگه دارید.',
  'Persisted text changed. Compare both versions below before replacing it.': 'متن ذخیره‌شده تغییر کرده است. پیش از جایگزینی، هر دو نسخهٔ زیر را مقایسه کنید.',
  'The edited block was deleted. Your text remains in the draft/recovery store; compare or copy it before discarding.': 'بلوک ویرایش‌شده حذف شده است. متن شما در پیش‌نویس یا بازیابی باقی مانده؛ پیش از دور ریختن، آن را مقایسه یا کپی کنید.',
  'Recovery cleanup failed — an older snapshot may appear on reopen.': 'پاک‌سازی بازیابی ناموفق بود؛ ممکن است پس از بازگشایی نسخه‌ای قدیمی نمایش داده شود.',
  'Wait for the current editor operation': 'منتظر پایان عملیات جاری ویرایشگر بمانید',
  'Close protection unavailable. Copy important text before closing.': 'محافظت هنگام بستن در دسترس نیست. پیش از بستن، متن مهم را کپی کنید.',
} as const;
export type MessageKey = keyof typeof messages;

export function translate(language: Language, key: MessageKey, values: Record<string, string | number> = {}): string {
  const text = language === 'fa' && Object.hasOwn(messages, key) ? messages[key] : key;
  return text.replace(/\{(\w+)\}/g, (_, name: string) => {
    const value = values[name];
    const formatted = String(value ?? `{${name}}`);
    return language === 'fa' && typeof value === 'number' ? formatted.replace(/\d/g, d => '۰۱۲۳۴۵۶۷۸۹'[Number(d)]) : formatted;
  });
}

export function statusText(language: Language, text: string): string {
  return Object.hasOwn(messages, text) ? translate(language, text as MessageKey) : text;
}

export function errorText(language: Language, text: string): string {
  if (language === 'en') return text;
  if (Object.hasOwn(messages, text)) return translate(language, text as MessageKey);
  const summary = text.replace(/^(?:invalid input|edit conflict|migration): /, '');
  if (Object.hasOwn(messages, summary)) return translate(language, summary as MessageKey);
  if (/remembered graph (?:has an unsupported schema|requires an explicit upgrade)|automatic.*schema|automatic.*migrat/i.test(text)) return translate(language, 'This graph needs an explicit open. Automatic reopening never upgrades old or unsupported graph formats.');
  if (/No such file|cannot find the file/i.test(text)) return translate(language, 'Graph file unavailable. Choose its current location; no replacement was created.');
  if (/compos/i.test(text)) return translate(language, 'Finish text composition before leaving the editor');
  if (/close protection/i.test(text)) return translate(language, 'Close protection unavailable. Copy important text before closing.');
  if (/busy|locked/i.test(text)) return translate(language, 'Database is busy. Wait for other writes to finish, then retry.');
  if (/read.only|disk|permission|readonly|full/i.test(text)) return translate(language, 'Could not save. Check available space and permissions; keep the app open.');
  if (/conflict|revision|changed|deleted|submission/i.test(text)) return translate(language, 'The persisted note differs. Copy the retained text if needed; it will not be overwritten.');
  return translate(language, 'Operation failed. Your text has not been discarded.');
}
