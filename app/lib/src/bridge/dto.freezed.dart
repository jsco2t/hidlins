// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'dto.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$CopyField {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CopyField);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CopyField()';
}


}

/// @nodoc
class $CopyFieldCopyWith<$Res>  {
$CopyFieldCopyWith(CopyField _, $Res Function(CopyField) __);
}


/// Adds pattern-matching-related methods to [CopyField].
extension CopyFieldPatterns on CopyField {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CopyField_Username value)?  username,TResult Function( CopyField_Password value)?  password,TResult Function( CopyField_TotpCode value)?  totpCode,TResult Function( CopyField_CustomField value)?  customField,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CopyField_Username() when username != null:
return username(_that);case CopyField_Password() when password != null:
return password(_that);case CopyField_TotpCode() when totpCode != null:
return totpCode(_that);case CopyField_CustomField() when customField != null:
return customField(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CopyField_Username value)  username,required TResult Function( CopyField_Password value)  password,required TResult Function( CopyField_TotpCode value)  totpCode,required TResult Function( CopyField_CustomField value)  customField,}){
final _that = this;
switch (_that) {
case CopyField_Username():
return username(_that);case CopyField_Password():
return password(_that);case CopyField_TotpCode():
return totpCode(_that);case CopyField_CustomField():
return customField(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CopyField_Username value)?  username,TResult? Function( CopyField_Password value)?  password,TResult? Function( CopyField_TotpCode value)?  totpCode,TResult? Function( CopyField_CustomField value)?  customField,}){
final _that = this;
switch (_that) {
case CopyField_Username() when username != null:
return username(_that);case CopyField_Password() when password != null:
return password(_that);case CopyField_TotpCode() when totpCode != null:
return totpCode(_that);case CopyField_CustomField() when customField != null:
return customField(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  username,TResult Function()?  password,TResult Function()?  totpCode,TResult Function( String field0)?  customField,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CopyField_Username() when username != null:
return username();case CopyField_Password() when password != null:
return password();case CopyField_TotpCode() when totpCode != null:
return totpCode();case CopyField_CustomField() when customField != null:
return customField(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  username,required TResult Function()  password,required TResult Function()  totpCode,required TResult Function( String field0)  customField,}) {final _that = this;
switch (_that) {
case CopyField_Username():
return username();case CopyField_Password():
return password();case CopyField_TotpCode():
return totpCode();case CopyField_CustomField():
return customField(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  username,TResult? Function()?  password,TResult? Function()?  totpCode,TResult? Function( String field0)?  customField,}) {final _that = this;
switch (_that) {
case CopyField_Username() when username != null:
return username();case CopyField_Password() when password != null:
return password();case CopyField_TotpCode() when totpCode != null:
return totpCode();case CopyField_CustomField() when customField != null:
return customField(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class CopyField_Username extends CopyField {
  const CopyField_Username(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CopyField_Username);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CopyField.username()';
}


}




/// @nodoc


class CopyField_Password extends CopyField {
  const CopyField_Password(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CopyField_Password);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CopyField.password()';
}


}




/// @nodoc


class CopyField_TotpCode extends CopyField {
  const CopyField_TotpCode(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CopyField_TotpCode);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'CopyField.totpCode()';
}


}




/// @nodoc


class CopyField_CustomField extends CopyField {
  const CopyField_CustomField(this.field0): super._();
  

 final  String field0;

/// Create a copy of CopyField
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CopyField_CustomFieldCopyWith<CopyField_CustomField> get copyWith => _$CopyField_CustomFieldCopyWithImpl<CopyField_CustomField>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CopyField_CustomField&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'CopyField.customField(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $CopyField_CustomFieldCopyWith<$Res> implements $CopyFieldCopyWith<$Res> {
  factory $CopyField_CustomFieldCopyWith(CopyField_CustomField value, $Res Function(CopyField_CustomField) _then) = _$CopyField_CustomFieldCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$CopyField_CustomFieldCopyWithImpl<$Res>
    implements $CopyField_CustomFieldCopyWith<$Res> {
  _$CopyField_CustomFieldCopyWithImpl(this._self, this._then);

  final CopyField_CustomField _self;
  final $Res Function(CopyField_CustomField) _then;

/// Create a copy of CopyField
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(CopyField_CustomField(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$KeyfileRef {

 Object get field0;



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is KeyfileRef&&const DeepCollectionEquality().equals(other.field0, field0));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(field0));



}

/// @nodoc
class $KeyfileRefCopyWith<$Res>  {
$KeyfileRefCopyWith(KeyfileRef _, $Res Function(KeyfileRef) __);
}


/// Adds pattern-matching-related methods to [KeyfileRef].
extension KeyfileRefPatterns on KeyfileRef {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( KeyfileRef_Path value)?  path,TResult Function( KeyfileRef_Bytes value)?  bytes,required TResult orElse(),}){
final _that = this;
switch (_that) {
case KeyfileRef_Path() when path != null:
return path(_that);case KeyfileRef_Bytes() when bytes != null:
return bytes(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( KeyfileRef_Path value)  path,required TResult Function( KeyfileRef_Bytes value)  bytes,}){
final _that = this;
switch (_that) {
case KeyfileRef_Path():
return path(_that);case KeyfileRef_Bytes():
return bytes(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( KeyfileRef_Path value)?  path,TResult? Function( KeyfileRef_Bytes value)?  bytes,}){
final _that = this;
switch (_that) {
case KeyfileRef_Path() when path != null:
return path(_that);case KeyfileRef_Bytes() when bytes != null:
return bytes(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String field0)?  path,TResult Function( Uint8List field0)?  bytes,required TResult orElse(),}) {final _that = this;
switch (_that) {
case KeyfileRef_Path() when path != null:
return path(_that.field0);case KeyfileRef_Bytes() when bytes != null:
return bytes(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String field0)  path,required TResult Function( Uint8List field0)  bytes,}) {final _that = this;
switch (_that) {
case KeyfileRef_Path():
return path(_that.field0);case KeyfileRef_Bytes():
return bytes(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String field0)?  path,TResult? Function( Uint8List field0)?  bytes,}) {final _that = this;
switch (_that) {
case KeyfileRef_Path() when path != null:
return path(_that.field0);case KeyfileRef_Bytes() when bytes != null:
return bytes(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class KeyfileRef_Path extends KeyfileRef {
  const KeyfileRef_Path(this.field0): super._();
  

@override final  String field0;

/// Create a copy of KeyfileRef
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$KeyfileRef_PathCopyWith<KeyfileRef_Path> get copyWith => _$KeyfileRef_PathCopyWithImpl<KeyfileRef_Path>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is KeyfileRef_Path&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);



}

/// @nodoc
abstract mixin class $KeyfileRef_PathCopyWith<$Res> implements $KeyfileRefCopyWith<$Res> {
  factory $KeyfileRef_PathCopyWith(KeyfileRef_Path value, $Res Function(KeyfileRef_Path) _then) = _$KeyfileRef_PathCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$KeyfileRef_PathCopyWithImpl<$Res>
    implements $KeyfileRef_PathCopyWith<$Res> {
  _$KeyfileRef_PathCopyWithImpl(this._self, this._then);

  final KeyfileRef_Path _self;
  final $Res Function(KeyfileRef_Path) _then;

/// Create a copy of KeyfileRef
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(KeyfileRef_Path(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class KeyfileRef_Bytes extends KeyfileRef {
  const KeyfileRef_Bytes(this.field0): super._();
  

@override final  Uint8List field0;

/// Create a copy of KeyfileRef
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$KeyfileRef_BytesCopyWith<KeyfileRef_Bytes> get copyWith => _$KeyfileRef_BytesCopyWithImpl<KeyfileRef_Bytes>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is KeyfileRef_Bytes&&const DeepCollectionEquality().equals(other.field0, field0));
}


@override
int get hashCode => Object.hash(runtimeType,const DeepCollectionEquality().hash(field0));



}

/// @nodoc
abstract mixin class $KeyfileRef_BytesCopyWith<$Res> implements $KeyfileRefCopyWith<$Res> {
  factory $KeyfileRef_BytesCopyWith(KeyfileRef_Bytes value, $Res Function(KeyfileRef_Bytes) _then) = _$KeyfileRef_BytesCopyWithImpl;
@useResult
$Res call({
 Uint8List field0
});




}
/// @nodoc
class _$KeyfileRef_BytesCopyWithImpl<$Res>
    implements $KeyfileRef_BytesCopyWith<$Res> {
  _$KeyfileRef_BytesCopyWithImpl(this._self, this._then);

  final KeyfileRef_Bytes _self;
  final $Res Function(KeyfileRef_Bytes) _then;

/// Create a copy of KeyfileRef
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(KeyfileRef_Bytes(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc
mixin _$RevealField {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is RevealField);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'RevealField()';
}


}

/// @nodoc
class $RevealFieldCopyWith<$Res>  {
$RevealFieldCopyWith(RevealField _, $Res Function(RevealField) __);
}


/// Adds pattern-matching-related methods to [RevealField].
extension RevealFieldPatterns on RevealField {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( RevealField_Password value)?  password,TResult Function( RevealField_CustomField value)?  customField,TResult Function( RevealField_TotpUri value)?  totpUri,required TResult orElse(),}){
final _that = this;
switch (_that) {
case RevealField_Password() when password != null:
return password(_that);case RevealField_CustomField() when customField != null:
return customField(_that);case RevealField_TotpUri() when totpUri != null:
return totpUri(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( RevealField_Password value)  password,required TResult Function( RevealField_CustomField value)  customField,required TResult Function( RevealField_TotpUri value)  totpUri,}){
final _that = this;
switch (_that) {
case RevealField_Password():
return password(_that);case RevealField_CustomField():
return customField(_that);case RevealField_TotpUri():
return totpUri(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( RevealField_Password value)?  password,TResult? Function( RevealField_CustomField value)?  customField,TResult? Function( RevealField_TotpUri value)?  totpUri,}){
final _that = this;
switch (_that) {
case RevealField_Password() when password != null:
return password(_that);case RevealField_CustomField() when customField != null:
return customField(_that);case RevealField_TotpUri() when totpUri != null:
return totpUri(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  password,TResult Function( String field0)?  customField,TResult Function()?  totpUri,required TResult orElse(),}) {final _that = this;
switch (_that) {
case RevealField_Password() when password != null:
return password();case RevealField_CustomField() when customField != null:
return customField(_that.field0);case RevealField_TotpUri() when totpUri != null:
return totpUri();case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  password,required TResult Function( String field0)  customField,required TResult Function()  totpUri,}) {final _that = this;
switch (_that) {
case RevealField_Password():
return password();case RevealField_CustomField():
return customField(_that.field0);case RevealField_TotpUri():
return totpUri();}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  password,TResult? Function( String field0)?  customField,TResult? Function()?  totpUri,}) {final _that = this;
switch (_that) {
case RevealField_Password() when password != null:
return password();case RevealField_CustomField() when customField != null:
return customField(_that.field0);case RevealField_TotpUri() when totpUri != null:
return totpUri();case _:
  return null;

}
}

}

/// @nodoc


class RevealField_Password extends RevealField {
  const RevealField_Password(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is RevealField_Password);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'RevealField.password()';
}


}




/// @nodoc


class RevealField_CustomField extends RevealField {
  const RevealField_CustomField(this.field0): super._();
  

 final  String field0;

/// Create a copy of RevealField
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$RevealField_CustomFieldCopyWith<RevealField_CustomField> get copyWith => _$RevealField_CustomFieldCopyWithImpl<RevealField_CustomField>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is RevealField_CustomField&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'RevealField.customField(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $RevealField_CustomFieldCopyWith<$Res> implements $RevealFieldCopyWith<$Res> {
  factory $RevealField_CustomFieldCopyWith(RevealField_CustomField value, $Res Function(RevealField_CustomField) _then) = _$RevealField_CustomFieldCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$RevealField_CustomFieldCopyWithImpl<$Res>
    implements $RevealField_CustomFieldCopyWith<$Res> {
  _$RevealField_CustomFieldCopyWithImpl(this._self, this._then);

  final RevealField_CustomField _self;
  final $Res Function(RevealField_CustomField) _then;

/// Create a copy of RevealField
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(RevealField_CustomField(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class RevealField_TotpUri extends RevealField {
  const RevealField_TotpUri(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is RevealField_TotpUri);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'RevealField.totpUri()';
}


}




/// @nodoc
mixin _$SearchScopeDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SearchScopeDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SearchScopeDto()';
}


}

/// @nodoc
class $SearchScopeDtoCopyWith<$Res>  {
$SearchScopeDtoCopyWith(SearchScopeDto _, $Res Function(SearchScopeDto) __);
}


/// Adds pattern-matching-related methods to [SearchScopeDto].
extension SearchScopeDtoPatterns on SearchScopeDto {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SearchScopeDto_All value)?  all,TResult Function( SearchScopeDto_GroupSubtree value)?  groupSubtree,TResult Function( SearchScopeDto_Tag value)?  tag,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SearchScopeDto_All() when all != null:
return all(_that);case SearchScopeDto_GroupSubtree() when groupSubtree != null:
return groupSubtree(_that);case SearchScopeDto_Tag() when tag != null:
return tag(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SearchScopeDto_All value)  all,required TResult Function( SearchScopeDto_GroupSubtree value)  groupSubtree,required TResult Function( SearchScopeDto_Tag value)  tag,}){
final _that = this;
switch (_that) {
case SearchScopeDto_All():
return all(_that);case SearchScopeDto_GroupSubtree():
return groupSubtree(_that);case SearchScopeDto_Tag():
return tag(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SearchScopeDto_All value)?  all,TResult? Function( SearchScopeDto_GroupSubtree value)?  groupSubtree,TResult? Function( SearchScopeDto_Tag value)?  tag,}){
final _that = this;
switch (_that) {
case SearchScopeDto_All() when all != null:
return all(_that);case SearchScopeDto_GroupSubtree() when groupSubtree != null:
return groupSubtree(_that);case SearchScopeDto_Tag() when tag != null:
return tag(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  all,TResult Function( String field0)?  groupSubtree,TResult Function( String field0)?  tag,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SearchScopeDto_All() when all != null:
return all();case SearchScopeDto_GroupSubtree() when groupSubtree != null:
return groupSubtree(_that.field0);case SearchScopeDto_Tag() when tag != null:
return tag(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  all,required TResult Function( String field0)  groupSubtree,required TResult Function( String field0)  tag,}) {final _that = this;
switch (_that) {
case SearchScopeDto_All():
return all();case SearchScopeDto_GroupSubtree():
return groupSubtree(_that.field0);case SearchScopeDto_Tag():
return tag(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  all,TResult? Function( String field0)?  groupSubtree,TResult? Function( String field0)?  tag,}) {final _that = this;
switch (_that) {
case SearchScopeDto_All() when all != null:
return all();case SearchScopeDto_GroupSubtree() when groupSubtree != null:
return groupSubtree(_that.field0);case SearchScopeDto_Tag() when tag != null:
return tag(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class SearchScopeDto_All extends SearchScopeDto {
  const SearchScopeDto_All(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SearchScopeDto_All);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SearchScopeDto.all()';
}


}




/// @nodoc


class SearchScopeDto_GroupSubtree extends SearchScopeDto {
  const SearchScopeDto_GroupSubtree(this.field0): super._();
  

 final  String field0;

/// Create a copy of SearchScopeDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SearchScopeDto_GroupSubtreeCopyWith<SearchScopeDto_GroupSubtree> get copyWith => _$SearchScopeDto_GroupSubtreeCopyWithImpl<SearchScopeDto_GroupSubtree>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SearchScopeDto_GroupSubtree&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'SearchScopeDto.groupSubtree(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $SearchScopeDto_GroupSubtreeCopyWith<$Res> implements $SearchScopeDtoCopyWith<$Res> {
  factory $SearchScopeDto_GroupSubtreeCopyWith(SearchScopeDto_GroupSubtree value, $Res Function(SearchScopeDto_GroupSubtree) _then) = _$SearchScopeDto_GroupSubtreeCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$SearchScopeDto_GroupSubtreeCopyWithImpl<$Res>
    implements $SearchScopeDto_GroupSubtreeCopyWith<$Res> {
  _$SearchScopeDto_GroupSubtreeCopyWithImpl(this._self, this._then);

  final SearchScopeDto_GroupSubtree _self;
  final $Res Function(SearchScopeDto_GroupSubtree) _then;

/// Create a copy of SearchScopeDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(SearchScopeDto_GroupSubtree(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class SearchScopeDto_Tag extends SearchScopeDto {
  const SearchScopeDto_Tag(this.field0): super._();
  

 final  String field0;

/// Create a copy of SearchScopeDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SearchScopeDto_TagCopyWith<SearchScopeDto_Tag> get copyWith => _$SearchScopeDto_TagCopyWithImpl<SearchScopeDto_Tag>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SearchScopeDto_Tag&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'SearchScopeDto.tag(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $SearchScopeDto_TagCopyWith<$Res> implements $SearchScopeDtoCopyWith<$Res> {
  factory $SearchScopeDto_TagCopyWith(SearchScopeDto_Tag value, $Res Function(SearchScopeDto_Tag) _then) = _$SearchScopeDto_TagCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$SearchScopeDto_TagCopyWithImpl<$Res>
    implements $SearchScopeDto_TagCopyWith<$Res> {
  _$SearchScopeDto_TagCopyWithImpl(this._self, this._then);

  final SearchScopeDto_Tag _self;
  final $Res Function(SearchScopeDto_Tag) _then;

/// Create a copy of SearchScopeDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(SearchScopeDto_Tag(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$SyncEvent {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncEvent);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncEvent()';
}


}

/// @nodoc
class $SyncEventCopyWith<$Res>  {
$SyncEventCopyWith(SyncEvent _, $Res Function(SyncEvent) __);
}


/// Adds pattern-matching-related methods to [SyncEvent].
extension SyncEventPatterns on SyncEvent {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SyncEvent_Started value)?  started,TResult Function( SyncEvent_Activity value)?  activity,TResult Function( SyncEvent_Done value)?  done,TResult Function( SyncEvent_Failed value)?  failed,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SyncEvent_Started() when started != null:
return started(_that);case SyncEvent_Activity() when activity != null:
return activity(_that);case SyncEvent_Done() when done != null:
return done(_that);case SyncEvent_Failed() when failed != null:
return failed(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SyncEvent_Started value)  started,required TResult Function( SyncEvent_Activity value)  activity,required TResult Function( SyncEvent_Done value)  done,required TResult Function( SyncEvent_Failed value)  failed,}){
final _that = this;
switch (_that) {
case SyncEvent_Started():
return started(_that);case SyncEvent_Activity():
return activity(_that);case SyncEvent_Done():
return done(_that);case SyncEvent_Failed():
return failed(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SyncEvent_Started value)?  started,TResult? Function( SyncEvent_Activity value)?  activity,TResult? Function( SyncEvent_Done value)?  done,TResult? Function( SyncEvent_Failed value)?  failed,}){
final _that = this;
switch (_that) {
case SyncEvent_Started() when started != null:
return started(_that);case SyncEvent_Activity() when activity != null:
return activity(_that);case SyncEvent_Done() when done != null:
return done(_that);case SyncEvent_Failed() when failed != null:
return failed(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  started,TResult Function()?  activity,TResult Function( SyncOutcomeDto field0)?  done,TResult Function( HidlinsApiError field0)?  failed,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SyncEvent_Started() when started != null:
return started();case SyncEvent_Activity() when activity != null:
return activity();case SyncEvent_Done() when done != null:
return done(_that.field0);case SyncEvent_Failed() when failed != null:
return failed(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  started,required TResult Function()  activity,required TResult Function( SyncOutcomeDto field0)  done,required TResult Function( HidlinsApiError field0)  failed,}) {final _that = this;
switch (_that) {
case SyncEvent_Started():
return started();case SyncEvent_Activity():
return activity();case SyncEvent_Done():
return done(_that.field0);case SyncEvent_Failed():
return failed(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  started,TResult? Function()?  activity,TResult? Function( SyncOutcomeDto field0)?  done,TResult? Function( HidlinsApiError field0)?  failed,}) {final _that = this;
switch (_that) {
case SyncEvent_Started() when started != null:
return started();case SyncEvent_Activity() when activity != null:
return activity();case SyncEvent_Done() when done != null:
return done(_that.field0);case SyncEvent_Failed() when failed != null:
return failed(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class SyncEvent_Started extends SyncEvent {
  const SyncEvent_Started(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncEvent_Started);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncEvent.started()';
}


}




/// @nodoc


class SyncEvent_Activity extends SyncEvent {
  const SyncEvent_Activity(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncEvent_Activity);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncEvent.activity()';
}


}




/// @nodoc


class SyncEvent_Done extends SyncEvent {
  const SyncEvent_Done(this.field0): super._();
  

 final  SyncOutcomeDto field0;

/// Create a copy of SyncEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SyncEvent_DoneCopyWith<SyncEvent_Done> get copyWith => _$SyncEvent_DoneCopyWithImpl<SyncEvent_Done>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncEvent_Done&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'SyncEvent.done(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $SyncEvent_DoneCopyWith<$Res> implements $SyncEventCopyWith<$Res> {
  factory $SyncEvent_DoneCopyWith(SyncEvent_Done value, $Res Function(SyncEvent_Done) _then) = _$SyncEvent_DoneCopyWithImpl;
@useResult
$Res call({
 SyncOutcomeDto field0
});


$SyncOutcomeDtoCopyWith<$Res> get field0;

}
/// @nodoc
class _$SyncEvent_DoneCopyWithImpl<$Res>
    implements $SyncEvent_DoneCopyWith<$Res> {
  _$SyncEvent_DoneCopyWithImpl(this._self, this._then);

  final SyncEvent_Done _self;
  final $Res Function(SyncEvent_Done) _then;

/// Create a copy of SyncEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(SyncEvent_Done(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as SyncOutcomeDto,
  ));
}

/// Create a copy of SyncEvent
/// with the given fields replaced by the non-null parameter values.
@override
@pragma('vm:prefer-inline')
$SyncOutcomeDtoCopyWith<$Res> get field0 {
  
  return $SyncOutcomeDtoCopyWith<$Res>(_self.field0, (value) {
    return _then(_self.copyWith(field0: value));
  });
}
}

/// @nodoc


class SyncEvent_Failed extends SyncEvent {
  const SyncEvent_Failed(this.field0): super._();
  

 final  HidlinsApiError field0;

/// Create a copy of SyncEvent
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SyncEvent_FailedCopyWith<SyncEvent_Failed> get copyWith => _$SyncEvent_FailedCopyWithImpl<SyncEvent_Failed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncEvent_Failed&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode => Object.hash(runtimeType,field0);

@override
String toString() {
  return 'SyncEvent.failed(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $SyncEvent_FailedCopyWith<$Res> implements $SyncEventCopyWith<$Res> {
  factory $SyncEvent_FailedCopyWith(SyncEvent_Failed value, $Res Function(SyncEvent_Failed) _then) = _$SyncEvent_FailedCopyWithImpl;
@useResult
$Res call({
 HidlinsApiError field0
});


$HidlinsApiErrorCopyWith<$Res> get field0;

}
/// @nodoc
class _$SyncEvent_FailedCopyWithImpl<$Res>
    implements $SyncEvent_FailedCopyWith<$Res> {
  _$SyncEvent_FailedCopyWithImpl(this._self, this._then);

  final SyncEvent_Failed _self;
  final $Res Function(SyncEvent_Failed) _then;

/// Create a copy of SyncEvent
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(SyncEvent_Failed(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as HidlinsApiError,
  ));
}

/// Create a copy of SyncEvent
/// with the given fields replaced by the non-null parameter values.
@override
@pragma('vm:prefer-inline')
$HidlinsApiErrorCopyWith<$Res> get field0 {
  
  return $HidlinsApiErrorCopyWith<$Res>(_self.field0, (value) {
    return _then(_self.copyWith(field0: value));
  });
}
}

/// @nodoc
mixin _$SyncOutcomeDto {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncOutcomeDto);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncOutcomeDto()';
}


}

/// @nodoc
class $SyncOutcomeDtoCopyWith<$Res>  {
$SyncOutcomeDtoCopyWith(SyncOutcomeDto _, $Res Function(SyncOutcomeDto) __);
}


/// Adds pattern-matching-related methods to [SyncOutcomeDto].
extension SyncOutcomeDtoPatterns on SyncOutcomeDto {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SyncOutcomeDto_AlreadyInSync value)?  alreadyInSync,TResult Function( SyncOutcomeDto_Pushed value)?  pushed,TResult Function( SyncOutcomeDto_FastReplaced value)?  fastReplaced,TResult Function( SyncOutcomeDto_Merged value)?  merged,TResult Function( SyncOutcomeDto_Unknown value)?  unknown,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SyncOutcomeDto_AlreadyInSync() when alreadyInSync != null:
return alreadyInSync(_that);case SyncOutcomeDto_Pushed() when pushed != null:
return pushed(_that);case SyncOutcomeDto_FastReplaced() when fastReplaced != null:
return fastReplaced(_that);case SyncOutcomeDto_Merged() when merged != null:
return merged(_that);case SyncOutcomeDto_Unknown() when unknown != null:
return unknown(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SyncOutcomeDto_AlreadyInSync value)  alreadyInSync,required TResult Function( SyncOutcomeDto_Pushed value)  pushed,required TResult Function( SyncOutcomeDto_FastReplaced value)  fastReplaced,required TResult Function( SyncOutcomeDto_Merged value)  merged,required TResult Function( SyncOutcomeDto_Unknown value)  unknown,}){
final _that = this;
switch (_that) {
case SyncOutcomeDto_AlreadyInSync():
return alreadyInSync(_that);case SyncOutcomeDto_Pushed():
return pushed(_that);case SyncOutcomeDto_FastReplaced():
return fastReplaced(_that);case SyncOutcomeDto_Merged():
return merged(_that);case SyncOutcomeDto_Unknown():
return unknown(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SyncOutcomeDto_AlreadyInSync value)?  alreadyInSync,TResult? Function( SyncOutcomeDto_Pushed value)?  pushed,TResult? Function( SyncOutcomeDto_FastReplaced value)?  fastReplaced,TResult? Function( SyncOutcomeDto_Merged value)?  merged,TResult? Function( SyncOutcomeDto_Unknown value)?  unknown,}){
final _that = this;
switch (_that) {
case SyncOutcomeDto_AlreadyInSync() when alreadyInSync != null:
return alreadyInSync(_that);case SyncOutcomeDto_Pushed() when pushed != null:
return pushed(_that);case SyncOutcomeDto_FastReplaced() when fastReplaced != null:
return fastReplaced(_that);case SyncOutcomeDto_Merged() when merged != null:
return merged(_that);case SyncOutcomeDto_Unknown() when unknown != null:
return unknown(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  alreadyInSync,TResult Function( bool isFirstSeed)?  pushed,TResult Function()?  fastReplaced,TResult Function( BigInt entriesAdded,  BigInt entriesModified,  BigInt entriesRemoved)?  merged,TResult Function()?  unknown,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SyncOutcomeDto_AlreadyInSync() when alreadyInSync != null:
return alreadyInSync();case SyncOutcomeDto_Pushed() when pushed != null:
return pushed(_that.isFirstSeed);case SyncOutcomeDto_FastReplaced() when fastReplaced != null:
return fastReplaced();case SyncOutcomeDto_Merged() when merged != null:
return merged(_that.entriesAdded,_that.entriesModified,_that.entriesRemoved);case SyncOutcomeDto_Unknown() when unknown != null:
return unknown();case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  alreadyInSync,required TResult Function( bool isFirstSeed)  pushed,required TResult Function()  fastReplaced,required TResult Function( BigInt entriesAdded,  BigInt entriesModified,  BigInt entriesRemoved)  merged,required TResult Function()  unknown,}) {final _that = this;
switch (_that) {
case SyncOutcomeDto_AlreadyInSync():
return alreadyInSync();case SyncOutcomeDto_Pushed():
return pushed(_that.isFirstSeed);case SyncOutcomeDto_FastReplaced():
return fastReplaced();case SyncOutcomeDto_Merged():
return merged(_that.entriesAdded,_that.entriesModified,_that.entriesRemoved);case SyncOutcomeDto_Unknown():
return unknown();}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  alreadyInSync,TResult? Function( bool isFirstSeed)?  pushed,TResult? Function()?  fastReplaced,TResult? Function( BigInt entriesAdded,  BigInt entriesModified,  BigInt entriesRemoved)?  merged,TResult? Function()?  unknown,}) {final _that = this;
switch (_that) {
case SyncOutcomeDto_AlreadyInSync() when alreadyInSync != null:
return alreadyInSync();case SyncOutcomeDto_Pushed() when pushed != null:
return pushed(_that.isFirstSeed);case SyncOutcomeDto_FastReplaced() when fastReplaced != null:
return fastReplaced();case SyncOutcomeDto_Merged() when merged != null:
return merged(_that.entriesAdded,_that.entriesModified,_that.entriesRemoved);case SyncOutcomeDto_Unknown() when unknown != null:
return unknown();case _:
  return null;

}
}

}

/// @nodoc


class SyncOutcomeDto_AlreadyInSync extends SyncOutcomeDto {
  const SyncOutcomeDto_AlreadyInSync(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncOutcomeDto_AlreadyInSync);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncOutcomeDto.alreadyInSync()';
}


}




/// @nodoc


class SyncOutcomeDto_Pushed extends SyncOutcomeDto {
  const SyncOutcomeDto_Pushed({required this.isFirstSeed}): super._();
  

 final  bool isFirstSeed;

/// Create a copy of SyncOutcomeDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SyncOutcomeDto_PushedCopyWith<SyncOutcomeDto_Pushed> get copyWith => _$SyncOutcomeDto_PushedCopyWithImpl<SyncOutcomeDto_Pushed>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncOutcomeDto_Pushed&&(identical(other.isFirstSeed, isFirstSeed) || other.isFirstSeed == isFirstSeed));
}


@override
int get hashCode => Object.hash(runtimeType,isFirstSeed);

@override
String toString() {
  return 'SyncOutcomeDto.pushed(isFirstSeed: $isFirstSeed)';
}


}

/// @nodoc
abstract mixin class $SyncOutcomeDto_PushedCopyWith<$Res> implements $SyncOutcomeDtoCopyWith<$Res> {
  factory $SyncOutcomeDto_PushedCopyWith(SyncOutcomeDto_Pushed value, $Res Function(SyncOutcomeDto_Pushed) _then) = _$SyncOutcomeDto_PushedCopyWithImpl;
@useResult
$Res call({
 bool isFirstSeed
});




}
/// @nodoc
class _$SyncOutcomeDto_PushedCopyWithImpl<$Res>
    implements $SyncOutcomeDto_PushedCopyWith<$Res> {
  _$SyncOutcomeDto_PushedCopyWithImpl(this._self, this._then);

  final SyncOutcomeDto_Pushed _self;
  final $Res Function(SyncOutcomeDto_Pushed) _then;

/// Create a copy of SyncOutcomeDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? isFirstSeed = null,}) {
  return _then(SyncOutcomeDto_Pushed(
isFirstSeed: null == isFirstSeed ? _self.isFirstSeed : isFirstSeed // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc


class SyncOutcomeDto_FastReplaced extends SyncOutcomeDto {
  const SyncOutcomeDto_FastReplaced(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncOutcomeDto_FastReplaced);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncOutcomeDto.fastReplaced()';
}


}




/// @nodoc


class SyncOutcomeDto_Merged extends SyncOutcomeDto {
  const SyncOutcomeDto_Merged({required this.entriesAdded, required this.entriesModified, required this.entriesRemoved}): super._();
  

 final  BigInt entriesAdded;
 final  BigInt entriesModified;
 final  BigInt entriesRemoved;

/// Create a copy of SyncOutcomeDto
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SyncOutcomeDto_MergedCopyWith<SyncOutcomeDto_Merged> get copyWith => _$SyncOutcomeDto_MergedCopyWithImpl<SyncOutcomeDto_Merged>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncOutcomeDto_Merged&&(identical(other.entriesAdded, entriesAdded) || other.entriesAdded == entriesAdded)&&(identical(other.entriesModified, entriesModified) || other.entriesModified == entriesModified)&&(identical(other.entriesRemoved, entriesRemoved) || other.entriesRemoved == entriesRemoved));
}


@override
int get hashCode => Object.hash(runtimeType,entriesAdded,entriesModified,entriesRemoved);

@override
String toString() {
  return 'SyncOutcomeDto.merged(entriesAdded: $entriesAdded, entriesModified: $entriesModified, entriesRemoved: $entriesRemoved)';
}


}

/// @nodoc
abstract mixin class $SyncOutcomeDto_MergedCopyWith<$Res> implements $SyncOutcomeDtoCopyWith<$Res> {
  factory $SyncOutcomeDto_MergedCopyWith(SyncOutcomeDto_Merged value, $Res Function(SyncOutcomeDto_Merged) _then) = _$SyncOutcomeDto_MergedCopyWithImpl;
@useResult
$Res call({
 BigInt entriesAdded, BigInt entriesModified, BigInt entriesRemoved
});




}
/// @nodoc
class _$SyncOutcomeDto_MergedCopyWithImpl<$Res>
    implements $SyncOutcomeDto_MergedCopyWith<$Res> {
  _$SyncOutcomeDto_MergedCopyWithImpl(this._self, this._then);

  final SyncOutcomeDto_Merged _self;
  final $Res Function(SyncOutcomeDto_Merged) _then;

/// Create a copy of SyncOutcomeDto
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? entriesAdded = null,Object? entriesModified = null,Object? entriesRemoved = null,}) {
  return _then(SyncOutcomeDto_Merged(
entriesAdded: null == entriesAdded ? _self.entriesAdded : entriesAdded // ignore: cast_nullable_to_non_nullable
as BigInt,entriesModified: null == entriesModified ? _self.entriesModified : entriesModified // ignore: cast_nullable_to_non_nullable
as BigInt,entriesRemoved: null == entriesRemoved ? _self.entriesRemoved : entriesRemoved // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc


class SyncOutcomeDto_Unknown extends SyncOutcomeDto {
  const SyncOutcomeDto_Unknown(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is SyncOutcomeDto_Unknown);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'SyncOutcomeDto.unknown()';
}


}




// dart format on
