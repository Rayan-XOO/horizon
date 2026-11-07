package dev.horizonlab.horizon;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract.Document;
import android.provider.DocumentsContract.Root;
import android.provider.DocumentsProvider;
import android.webkit.MimeTypeMap;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/** Shows the game folder (getExternalFilesDir) as a "Horizon" root in the system Files app. */
public class HorizonDocumentsProvider extends DocumentsProvider {
    private static final String ROOT_ID = "horizon";
    private static final String PREFIX = "root:";

    private static final String[] DEFAULT_ROOT_PROJECTION = {
        Root.COLUMN_ROOT_ID, Root.COLUMN_FLAGS, Root.COLUMN_TITLE, Root.COLUMN_ICON,
        Root.COLUMN_DOCUMENT_ID, Root.COLUMN_AVAILABLE_BYTES,
    };
    private static final String[] DEFAULT_DOCUMENT_PROJECTION = {
        Document.COLUMN_DOCUMENT_ID, Document.COLUMN_MIME_TYPE, Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_LAST_MODIFIED, Document.COLUMN_FLAGS, Document.COLUMN_SIZE,
    };

    @Override
    public boolean onCreate() {
        return true;
    }

    private File baseDir() throws FileNotFoundException {
        File dir = getContext().getExternalFilesDir(null);
        if (dir == null) throw new FileNotFoundException("external storage unavailable");
        dir.mkdirs();
        try {
            return dir.getCanonicalFile();
        } catch (IOException e) {
            return dir.getAbsoluteFile();
        }
    }

    private static boolean inside(File base, File f) {
        String b = base.getPath();
        String p = f.getPath();
        return p.equals(b) || p.startsWith(b.endsWith("/") ? b : b + "/");
    }

    private File fileFor(String docId) throws FileNotFoundException {
        if (docId == null || !docId.startsWith(PREFIX)) throw new FileNotFoundException("bad id: " + docId);
        String rel = docId.substring(PREFIX.length());
        File base = baseDir();
        File f = rel.isEmpty() ? base : new File(base, rel);
        try {
            f = f.getCanonicalFile();
        } catch (IOException e) {
            throw new FileNotFoundException("bad path: " + docId);
        }
        if (!inside(base, f)) throw new FileNotFoundException("outside root: " + docId);
        return f;
    }

    private String idFor(File f) throws FileNotFoundException {
        File base = baseDir();
        String b = base.getPath();
        String p = f.getAbsolutePath();
        try {
            p = f.getCanonicalPath();
        } catch (IOException ignored) {
        }
        if (p.equals(b)) return PREFIX;
        if (!inside(base, new File(p))) throw new FileNotFoundException("outside root");
        return PREFIX + p.substring(b.endsWith("/") ? b.length() : b.length() + 1);
    }

    private static String mimeFor(File f) {
        if (f.isDirectory()) return Document.MIME_TYPE_DIR;
        String name = f.getName();
        int dot = name.lastIndexOf('.');
        if (dot >= 0) {
            String m = MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substring(dot + 1).toLowerCase());
            if (m != null) return m;
        }
        return "application/octet-stream";
    }

    private void addRow(MatrixCursor c, String docId, File f) throws FileNotFoundException {
        int flags;
        if (f.isDirectory()) {
            flags = Document.FLAG_DIR_SUPPORTS_CREATE;
        } else {
            flags = Document.FLAG_SUPPORTS_WRITE;
        }
        boolean isRoot = f.equals(baseDir());
        if (!isRoot) flags |= Document.FLAG_SUPPORTS_DELETE | Document.FLAG_SUPPORTS_RENAME;
        MatrixCursor.RowBuilder row = c.newRow();
        row.add(Document.COLUMN_DOCUMENT_ID, docId);
        row.add(Document.COLUMN_MIME_TYPE, mimeFor(f));
        row.add(Document.COLUMN_DISPLAY_NAME, isRoot ? "Horizon" : f.getName());
        row.add(Document.COLUMN_LAST_MODIFIED, f.lastModified());
        row.add(Document.COLUMN_FLAGS, flags);
        row.add(Document.COLUMN_SIZE, f.isDirectory() ? null : f.length());
    }

    @Override
    public Cursor queryRoots(String[] projection) throws FileNotFoundException {
        MatrixCursor c = new MatrixCursor(projection != null ? projection : DEFAULT_ROOT_PROJECTION);
        File base = baseDir();
        MatrixCursor.RowBuilder row = c.newRow();
        row.add(Root.COLUMN_ROOT_ID, ROOT_ID);
        row.add(Root.COLUMN_FLAGS, Root.FLAG_SUPPORTS_CREATE | Root.FLAG_SUPPORTS_IS_CHILD | Root.FLAG_LOCAL_ONLY);
        row.add(Root.COLUMN_TITLE, "Horizon");
        row.add(Root.COLUMN_ICON, getContext().getApplicationInfo().icon);
        row.add(Root.COLUMN_DOCUMENT_ID, PREFIX);
        row.add(Root.COLUMN_AVAILABLE_BYTES, base.getUsableSpace());
        return c;
    }

    @Override
    public Cursor queryDocument(String documentId, String[] projection) throws FileNotFoundException {
        MatrixCursor c = new MatrixCursor(projection != null ? projection : DEFAULT_DOCUMENT_PROJECTION);
        File f = fileFor(documentId);
        if (!f.exists()) throw new FileNotFoundException(documentId);
        addRow(c, idFor(f), f);
        return c;
    }

    @Override
    public Cursor queryChildDocuments(String parentDocumentId, String[] projection, String sortOrder)
            throws FileNotFoundException {
        MatrixCursor c = new MatrixCursor(projection != null ? projection : DEFAULT_DOCUMENT_PROJECTION);
        File dir = fileFor(parentDocumentId);
        File[] kids = dir.listFiles();
        if (kids != null) {
            File base = baseDir();
            for (File k : kids) {
                File ck;
                try {
                    ck = k.getCanonicalFile();
                } catch (IOException e) {
                    continue;
                }
                if (!inside(base, ck)) continue;
                addRow(c, idFor(ck), ck);
            }
        }
        return c;
    }

    @Override
    public ParcelFileDescriptor openDocument(String documentId, String mode, CancellationSignal signal)
            throws FileNotFoundException {
        File f = fileFor(documentId);
        if (f.isDirectory()) throw new FileNotFoundException("is a directory");
        return ParcelFileDescriptor.open(f, ParcelFileDescriptor.parseMode(mode));
    }

    @Override
    public String createDocument(String parentDocumentId, String mimeType, String displayName)
            throws FileNotFoundException {
        File parent = fileFor(parentDocumentId);
        if (!parent.isDirectory()) throw new FileNotFoundException("parent is not a directory");
        String name = displayName == null ? "" : displayName.replace('/', '_');
        if (name.isEmpty() || name.equals(".") || name.equals("..")) name = "new";
        File f = new File(parent, name);
        int n = 1;
        String stem = name, ext = "";
        int dot = name.lastIndexOf('.');
        if (dot > 0 && !Document.MIME_TYPE_DIR.equals(mimeType)) {
            stem = name.substring(0, dot);
            ext = name.substring(dot);
        }
        while (f.exists()) f = new File(parent, stem + " (" + (n++) + ")" + ext);
        boolean ok;
        try {
            ok = Document.MIME_TYPE_DIR.equals(mimeType) ? f.mkdir() : f.createNewFile();
        } catch (IOException e) {
            throw new FileNotFoundException("create failed: " + e.getMessage());
        }
        if (!ok) throw new FileNotFoundException("create failed");
        return idFor(f);
    }

    private static boolean deleteRec(File f) {
        if (f.isDirectory()) {
            File[] kids = f.listFiles();
            if (kids != null) for (File k : kids) deleteRec(k);
        }
        return f.delete();
    }

    @Override
    public void deleteDocument(String documentId) throws FileNotFoundException {
        File f = fileFor(documentId);
        if (f.equals(baseDir())) throw new FileNotFoundException("can't delete root");
        if (!deleteRec(f)) throw new FileNotFoundException("delete failed");
    }

    @Override
    public String renameDocument(String documentId, String displayName) throws FileNotFoundException {
        File f = fileFor(documentId);
        if (f.equals(baseDir())) throw new FileNotFoundException("can't rename root");
        if (displayName == null || displayName.isEmpty() || displayName.contains("/")
                || displayName.equals(".") || displayName.equals("..")) {
            throw new FileNotFoundException("bad name");
        }
        File target = new File(f.getParentFile(), displayName);
        if (target.exists()) throw new FileNotFoundException("already exists");
        if (!f.renameTo(target)) throw new FileNotFoundException("rename failed");
        return idFor(target);
    }

    @Override
    public boolean isChildDocument(String parentDocumentId, String documentId) {
        try {
            return inside(fileFor(parentDocumentId), fileFor(documentId));
        } catch (FileNotFoundException e) {
            return false;
        }
    }

    @Override
    public String getDocumentType(String documentId) throws FileNotFoundException {
        return mimeFor(fileFor(documentId));
    }
}
