use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

pub(crate) fn objc_to_json(obj: &AnyObject) -> Result<serde_json::Value, String> {
    let cls_ns_null = AnyClass::get(c"NSNull");
    let cls_ns_number = AnyClass::get(c"NSNumber");
    let cls_ns_string = AnyClass::get(c"NSString");
    let cls_ns_array = AnyClass::get(c"NSArray");
    let cls_ns_dict = AnyClass::get(c"NSDictionary");

    let is_kind = |cls: Option<&AnyClass>| -> bool {
        let Some(cls) = cls else {
            return false;
        };
        unsafe { msg_send![obj, isKindOfClass: cls] }
    };

    if is_kind(cls_ns_null) {
        return Ok(serde_json::Value::Null);
    }
    if is_kind(cls_ns_number) {
        return number_to_json(obj);
    }
    if is_kind(cls_ns_string) {
        let s: &NSString = unsafe { &*(obj as *const AnyObject as *const NSString) };
        return Ok(serde_json::Value::String(s.to_string()));
    }
    if is_kind(cls_ns_array) {
        return array_to_json(obj);
    }
    if is_kind(cls_ns_dict) {
        return dictionary_to_json(obj);
    }

    let cls_name = unsafe {
        let cls: *const AnyClass = msg_send![obj, class];
        if cls.is_null() {
            "<null>".to_string()
        } else {
            (*cls).name().to_string_lossy().into_owned()
        }
    };
    Err(format!("unsupported objc type (class = {cls_name})"))
}

fn number_to_json(obj: &AnyObject) -> Result<serde_json::Value, String> {
    let num: &NSNumber = unsafe { &*(obj as *const AnyObject as *const NSNumber) };
    let enc = unsafe {
        let ptr: *const std::os::raw::c_char = msg_send![num, objCType];
        if ptr.is_null() {
            return Err("NSNumber objCType null".into());
        }
        std::ffi::CStr::from_ptr(ptr)
    };
    let enc_bytes = enc.to_bytes();
    if enc_bytes == b"c" || enc_bytes == b"B" {
        return Ok(serde_json::Value::Bool(num.as_bool()));
    }
    if matches!(enc_bytes, b"f" | b"d") {
        let f = num.as_f64();
        return Ok(serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null));
    }
    Ok(serde_json::Value::Number(serde_json::Number::from(
        num.as_i64(),
    )))
}

fn array_to_json(obj: &AnyObject) -> Result<serde_json::Value, String> {
    let arr: &objc2_foundation::NSArray =
        unsafe { &*(obj as *const AnyObject as *const objc2_foundation::NSArray) };
    let count: usize = arr.count();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let item: Retained<AnyObject> = unsafe { msg_send![arr, objectAtIndex: i] };
        out.push(objc_to_json(&item)?);
    }
    Ok(serde_json::Value::Array(out))
}

fn dictionary_to_json(obj: &AnyObject) -> Result<serde_json::Value, String> {
    let dict: &NSDictionary = unsafe { &*(obj as *const AnyObject as *const NSDictionary) };
    let keys: Retained<objc2_foundation::NSArray> = unsafe { msg_send![dict, allKeys] };
    let key_count: usize = keys.count();
    let mut map = serde_json::Map::with_capacity(key_count);
    for i in 0..key_count {
        let key_obj: Retained<AnyObject> = unsafe { msg_send![&*keys, objectAtIndex: i] };
        let key_str: &NSString = match key_obj.downcast_ref::<NSString>() {
            Some(s) => s,
            None => continue,
        };
        let key = key_str.to_string();
        let value_obj: Option<Retained<AnyObject>> =
            unsafe { msg_send![dict, objectForKey: &*key_obj] };
        if let Some(v) = value_obj {
            map.insert(key, objc_to_json(&v)?);
        }
    }
    Ok(serde_json::Value::Object(map))
}
