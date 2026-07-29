// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'error.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$HidlinsApiError {





@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError()';
}


}

/// @nodoc
class $HidlinsApiErrorCopyWith<$Res>  {
$HidlinsApiErrorCopyWith(HidlinsApiError _, $Res Function(HidlinsApiError) __);
}


/// Adds pattern-matching-related methods to [HidlinsApiError].
extension HidlinsApiErrorPatterns on HidlinsApiError {
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

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( HidlinsApiError_AuthenticationFailed value)?  authenticationFailed,TResult Function( HidlinsApiError_VaultLocked value)?  vaultLocked,TResult Function( HidlinsApiError_VaultBusySyncing value)?  vaultBusySyncing,TResult Function( HidlinsApiError_VaultContended value)?  vaultContended,TResult Function( HidlinsApiError_PathExists value)?  pathExists,TResult Function( HidlinsApiError_FileNotFound value)?  fileNotFound,TResult Function( HidlinsApiError_KeyfileRequired value)?  keyfileRequired,TResult Function( HidlinsApiError_RegistryChanged value)?  registryChanged,TResult Function( HidlinsApiError_InvalidFormat value)?  invalidFormat,TResult Function( HidlinsApiError_RegistryMalformed value)?  registryMalformed,TResult Function( HidlinsApiError_SyncNotConfigured value)?  syncNotConfigured,TResult Function( HidlinsApiError_SyncRemoteUnreachable value)?  syncRemoteUnreachable,TResult Function( HidlinsApiError_SyncAuthFailed value)?  syncAuthFailed,TResult Function( HidlinsApiError_SyncConflictUnresolvable value)?  syncConflictUnresolvable,TResult Function( HidlinsApiError_SyncDuplicateTarget value)?  syncDuplicateTarget,TResult Function( HidlinsApiError_InvalidInput value)?  invalidInput,TResult Function( HidlinsApiError_Io value)?  io,TResult Function( HidlinsApiError_Internal value)?  internal,required TResult orElse(),}){
final _that = this;
switch (_that) {
case HidlinsApiError_AuthenticationFailed() when authenticationFailed != null:
return authenticationFailed(_that);case HidlinsApiError_VaultLocked() when vaultLocked != null:
return vaultLocked(_that);case HidlinsApiError_VaultBusySyncing() when vaultBusySyncing != null:
return vaultBusySyncing(_that);case HidlinsApiError_VaultContended() when vaultContended != null:
return vaultContended(_that);case HidlinsApiError_PathExists() when pathExists != null:
return pathExists(_that);case HidlinsApiError_FileNotFound() when fileNotFound != null:
return fileNotFound(_that);case HidlinsApiError_KeyfileRequired() when keyfileRequired != null:
return keyfileRequired(_that);case HidlinsApiError_RegistryChanged() when registryChanged != null:
return registryChanged(_that);case HidlinsApiError_InvalidFormat() when invalidFormat != null:
return invalidFormat(_that);case HidlinsApiError_RegistryMalformed() when registryMalformed != null:
return registryMalformed(_that);case HidlinsApiError_SyncNotConfigured() when syncNotConfigured != null:
return syncNotConfigured(_that);case HidlinsApiError_SyncRemoteUnreachable() when syncRemoteUnreachable != null:
return syncRemoteUnreachable(_that);case HidlinsApiError_SyncAuthFailed() when syncAuthFailed != null:
return syncAuthFailed(_that);case HidlinsApiError_SyncConflictUnresolvable() when syncConflictUnresolvable != null:
return syncConflictUnresolvable(_that);case HidlinsApiError_SyncDuplicateTarget() when syncDuplicateTarget != null:
return syncDuplicateTarget(_that);case HidlinsApiError_InvalidInput() when invalidInput != null:
return invalidInput(_that);case HidlinsApiError_Io() when io != null:
return io(_that);case HidlinsApiError_Internal() when internal != null:
return internal(_that);case _:
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

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( HidlinsApiError_AuthenticationFailed value)  authenticationFailed,required TResult Function( HidlinsApiError_VaultLocked value)  vaultLocked,required TResult Function( HidlinsApiError_VaultBusySyncing value)  vaultBusySyncing,required TResult Function( HidlinsApiError_VaultContended value)  vaultContended,required TResult Function( HidlinsApiError_PathExists value)  pathExists,required TResult Function( HidlinsApiError_FileNotFound value)  fileNotFound,required TResult Function( HidlinsApiError_KeyfileRequired value)  keyfileRequired,required TResult Function( HidlinsApiError_RegistryChanged value)  registryChanged,required TResult Function( HidlinsApiError_InvalidFormat value)  invalidFormat,required TResult Function( HidlinsApiError_RegistryMalformed value)  registryMalformed,required TResult Function( HidlinsApiError_SyncNotConfigured value)  syncNotConfigured,required TResult Function( HidlinsApiError_SyncRemoteUnreachable value)  syncRemoteUnreachable,required TResult Function( HidlinsApiError_SyncAuthFailed value)  syncAuthFailed,required TResult Function( HidlinsApiError_SyncConflictUnresolvable value)  syncConflictUnresolvable,required TResult Function( HidlinsApiError_SyncDuplicateTarget value)  syncDuplicateTarget,required TResult Function( HidlinsApiError_InvalidInput value)  invalidInput,required TResult Function( HidlinsApiError_Io value)  io,required TResult Function( HidlinsApiError_Internal value)  internal,}){
final _that = this;
switch (_that) {
case HidlinsApiError_AuthenticationFailed():
return authenticationFailed(_that);case HidlinsApiError_VaultLocked():
return vaultLocked(_that);case HidlinsApiError_VaultBusySyncing():
return vaultBusySyncing(_that);case HidlinsApiError_VaultContended():
return vaultContended(_that);case HidlinsApiError_PathExists():
return pathExists(_that);case HidlinsApiError_FileNotFound():
return fileNotFound(_that);case HidlinsApiError_KeyfileRequired():
return keyfileRequired(_that);case HidlinsApiError_RegistryChanged():
return registryChanged(_that);case HidlinsApiError_InvalidFormat():
return invalidFormat(_that);case HidlinsApiError_RegistryMalformed():
return registryMalformed(_that);case HidlinsApiError_SyncNotConfigured():
return syncNotConfigured(_that);case HidlinsApiError_SyncRemoteUnreachable():
return syncRemoteUnreachable(_that);case HidlinsApiError_SyncAuthFailed():
return syncAuthFailed(_that);case HidlinsApiError_SyncConflictUnresolvable():
return syncConflictUnresolvable(_that);case HidlinsApiError_SyncDuplicateTarget():
return syncDuplicateTarget(_that);case HidlinsApiError_InvalidInput():
return invalidInput(_that);case HidlinsApiError_Io():
return io(_that);case HidlinsApiError_Internal():
return internal(_that);}
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

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( HidlinsApiError_AuthenticationFailed value)?  authenticationFailed,TResult? Function( HidlinsApiError_VaultLocked value)?  vaultLocked,TResult? Function( HidlinsApiError_VaultBusySyncing value)?  vaultBusySyncing,TResult? Function( HidlinsApiError_VaultContended value)?  vaultContended,TResult? Function( HidlinsApiError_PathExists value)?  pathExists,TResult? Function( HidlinsApiError_FileNotFound value)?  fileNotFound,TResult? Function( HidlinsApiError_KeyfileRequired value)?  keyfileRequired,TResult? Function( HidlinsApiError_RegistryChanged value)?  registryChanged,TResult? Function( HidlinsApiError_InvalidFormat value)?  invalidFormat,TResult? Function( HidlinsApiError_RegistryMalformed value)?  registryMalformed,TResult? Function( HidlinsApiError_SyncNotConfigured value)?  syncNotConfigured,TResult? Function( HidlinsApiError_SyncRemoteUnreachable value)?  syncRemoteUnreachable,TResult? Function( HidlinsApiError_SyncAuthFailed value)?  syncAuthFailed,TResult? Function( HidlinsApiError_SyncConflictUnresolvable value)?  syncConflictUnresolvable,TResult? Function( HidlinsApiError_SyncDuplicateTarget value)?  syncDuplicateTarget,TResult? Function( HidlinsApiError_InvalidInput value)?  invalidInput,TResult? Function( HidlinsApiError_Io value)?  io,TResult? Function( HidlinsApiError_Internal value)?  internal,}){
final _that = this;
switch (_that) {
case HidlinsApiError_AuthenticationFailed() when authenticationFailed != null:
return authenticationFailed(_that);case HidlinsApiError_VaultLocked() when vaultLocked != null:
return vaultLocked(_that);case HidlinsApiError_VaultBusySyncing() when vaultBusySyncing != null:
return vaultBusySyncing(_that);case HidlinsApiError_VaultContended() when vaultContended != null:
return vaultContended(_that);case HidlinsApiError_PathExists() when pathExists != null:
return pathExists(_that);case HidlinsApiError_FileNotFound() when fileNotFound != null:
return fileNotFound(_that);case HidlinsApiError_KeyfileRequired() when keyfileRequired != null:
return keyfileRequired(_that);case HidlinsApiError_RegistryChanged() when registryChanged != null:
return registryChanged(_that);case HidlinsApiError_InvalidFormat() when invalidFormat != null:
return invalidFormat(_that);case HidlinsApiError_RegistryMalformed() when registryMalformed != null:
return registryMalformed(_that);case HidlinsApiError_SyncNotConfigured() when syncNotConfigured != null:
return syncNotConfigured(_that);case HidlinsApiError_SyncRemoteUnreachable() when syncRemoteUnreachable != null:
return syncRemoteUnreachable(_that);case HidlinsApiError_SyncAuthFailed() when syncAuthFailed != null:
return syncAuthFailed(_that);case HidlinsApiError_SyncConflictUnresolvable() when syncConflictUnresolvable != null:
return syncConflictUnresolvable(_that);case HidlinsApiError_SyncDuplicateTarget() when syncDuplicateTarget != null:
return syncDuplicateTarget(_that);case HidlinsApiError_InvalidInput() when invalidInput != null:
return invalidInput(_that);case HidlinsApiError_Io() when io != null:
return io(_that);case HidlinsApiError_Internal() when internal != null:
return internal(_that);case _:
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

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  authenticationFailed,TResult Function()?  vaultLocked,TResult Function()?  vaultBusySyncing,TResult Function( int? holderPid)?  vaultContended,TResult Function( String path)?  pathExists,TResult Function( String path)?  fileNotFound,TResult Function()?  keyfileRequired,TResult Function()?  registryChanged,TResult Function()?  invalidFormat,TResult Function()?  registryMalformed,TResult Function()?  syncNotConfigured,TResult Function( String? endpoint)?  syncRemoteUnreachable,TResult Function()?  syncAuthFailed,TResult Function( String backupPath)?  syncConflictUnresolvable,TResult Function( String existingVault)?  syncDuplicateTarget,TResult Function( String field,  String reason)?  invalidInput,TResult Function( String context)?  io,TResult Function( String context)?  internal,required TResult orElse(),}) {final _that = this;
switch (_that) {
case HidlinsApiError_AuthenticationFailed() when authenticationFailed != null:
return authenticationFailed();case HidlinsApiError_VaultLocked() when vaultLocked != null:
return vaultLocked();case HidlinsApiError_VaultBusySyncing() when vaultBusySyncing != null:
return vaultBusySyncing();case HidlinsApiError_VaultContended() when vaultContended != null:
return vaultContended(_that.holderPid);case HidlinsApiError_PathExists() when pathExists != null:
return pathExists(_that.path);case HidlinsApiError_FileNotFound() when fileNotFound != null:
return fileNotFound(_that.path);case HidlinsApiError_KeyfileRequired() when keyfileRequired != null:
return keyfileRequired();case HidlinsApiError_RegistryChanged() when registryChanged != null:
return registryChanged();case HidlinsApiError_InvalidFormat() when invalidFormat != null:
return invalidFormat();case HidlinsApiError_RegistryMalformed() when registryMalformed != null:
return registryMalformed();case HidlinsApiError_SyncNotConfigured() when syncNotConfigured != null:
return syncNotConfigured();case HidlinsApiError_SyncRemoteUnreachable() when syncRemoteUnreachable != null:
return syncRemoteUnreachable(_that.endpoint);case HidlinsApiError_SyncAuthFailed() when syncAuthFailed != null:
return syncAuthFailed();case HidlinsApiError_SyncConflictUnresolvable() when syncConflictUnresolvable != null:
return syncConflictUnresolvable(_that.backupPath);case HidlinsApiError_SyncDuplicateTarget() when syncDuplicateTarget != null:
return syncDuplicateTarget(_that.existingVault);case HidlinsApiError_InvalidInput() when invalidInput != null:
return invalidInput(_that.field,_that.reason);case HidlinsApiError_Io() when io != null:
return io(_that.context);case HidlinsApiError_Internal() when internal != null:
return internal(_that.context);case _:
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

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  authenticationFailed,required TResult Function()  vaultLocked,required TResult Function()  vaultBusySyncing,required TResult Function( int? holderPid)  vaultContended,required TResult Function( String path)  pathExists,required TResult Function( String path)  fileNotFound,required TResult Function()  keyfileRequired,required TResult Function()  registryChanged,required TResult Function()  invalidFormat,required TResult Function()  registryMalformed,required TResult Function()  syncNotConfigured,required TResult Function( String? endpoint)  syncRemoteUnreachable,required TResult Function()  syncAuthFailed,required TResult Function( String backupPath)  syncConflictUnresolvable,required TResult Function( String existingVault)  syncDuplicateTarget,required TResult Function( String field,  String reason)  invalidInput,required TResult Function( String context)  io,required TResult Function( String context)  internal,}) {final _that = this;
switch (_that) {
case HidlinsApiError_AuthenticationFailed():
return authenticationFailed();case HidlinsApiError_VaultLocked():
return vaultLocked();case HidlinsApiError_VaultBusySyncing():
return vaultBusySyncing();case HidlinsApiError_VaultContended():
return vaultContended(_that.holderPid);case HidlinsApiError_PathExists():
return pathExists(_that.path);case HidlinsApiError_FileNotFound():
return fileNotFound(_that.path);case HidlinsApiError_KeyfileRequired():
return keyfileRequired();case HidlinsApiError_RegistryChanged():
return registryChanged();case HidlinsApiError_InvalidFormat():
return invalidFormat();case HidlinsApiError_RegistryMalformed():
return registryMalformed();case HidlinsApiError_SyncNotConfigured():
return syncNotConfigured();case HidlinsApiError_SyncRemoteUnreachable():
return syncRemoteUnreachable(_that.endpoint);case HidlinsApiError_SyncAuthFailed():
return syncAuthFailed();case HidlinsApiError_SyncConflictUnresolvable():
return syncConflictUnresolvable(_that.backupPath);case HidlinsApiError_SyncDuplicateTarget():
return syncDuplicateTarget(_that.existingVault);case HidlinsApiError_InvalidInput():
return invalidInput(_that.field,_that.reason);case HidlinsApiError_Io():
return io(_that.context);case HidlinsApiError_Internal():
return internal(_that.context);}
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

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  authenticationFailed,TResult? Function()?  vaultLocked,TResult? Function()?  vaultBusySyncing,TResult? Function( int? holderPid)?  vaultContended,TResult? Function( String path)?  pathExists,TResult? Function( String path)?  fileNotFound,TResult? Function()?  keyfileRequired,TResult? Function()?  registryChanged,TResult? Function()?  invalidFormat,TResult? Function()?  registryMalformed,TResult? Function()?  syncNotConfigured,TResult? Function( String? endpoint)?  syncRemoteUnreachable,TResult? Function()?  syncAuthFailed,TResult? Function( String backupPath)?  syncConflictUnresolvable,TResult? Function( String existingVault)?  syncDuplicateTarget,TResult? Function( String field,  String reason)?  invalidInput,TResult? Function( String context)?  io,TResult? Function( String context)?  internal,}) {final _that = this;
switch (_that) {
case HidlinsApiError_AuthenticationFailed() when authenticationFailed != null:
return authenticationFailed();case HidlinsApiError_VaultLocked() when vaultLocked != null:
return vaultLocked();case HidlinsApiError_VaultBusySyncing() when vaultBusySyncing != null:
return vaultBusySyncing();case HidlinsApiError_VaultContended() when vaultContended != null:
return vaultContended(_that.holderPid);case HidlinsApiError_PathExists() when pathExists != null:
return pathExists(_that.path);case HidlinsApiError_FileNotFound() when fileNotFound != null:
return fileNotFound(_that.path);case HidlinsApiError_KeyfileRequired() when keyfileRequired != null:
return keyfileRequired();case HidlinsApiError_RegistryChanged() when registryChanged != null:
return registryChanged();case HidlinsApiError_InvalidFormat() when invalidFormat != null:
return invalidFormat();case HidlinsApiError_RegistryMalformed() when registryMalformed != null:
return registryMalformed();case HidlinsApiError_SyncNotConfigured() when syncNotConfigured != null:
return syncNotConfigured();case HidlinsApiError_SyncRemoteUnreachable() when syncRemoteUnreachable != null:
return syncRemoteUnreachable(_that.endpoint);case HidlinsApiError_SyncAuthFailed() when syncAuthFailed != null:
return syncAuthFailed();case HidlinsApiError_SyncConflictUnresolvable() when syncConflictUnresolvable != null:
return syncConflictUnresolvable(_that.backupPath);case HidlinsApiError_SyncDuplicateTarget() when syncDuplicateTarget != null:
return syncDuplicateTarget(_that.existingVault);case HidlinsApiError_InvalidInput() when invalidInput != null:
return invalidInput(_that.field,_that.reason);case HidlinsApiError_Io() when io != null:
return io(_that.context);case HidlinsApiError_Internal() when internal != null:
return internal(_that.context);case _:
  return null;

}
}

}

/// @nodoc


class HidlinsApiError_AuthenticationFailed extends HidlinsApiError {
  const HidlinsApiError_AuthenticationFailed(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_AuthenticationFailed);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.authenticationFailed()';
}


}




/// @nodoc


class HidlinsApiError_VaultLocked extends HidlinsApiError {
  const HidlinsApiError_VaultLocked(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_VaultLocked);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.vaultLocked()';
}


}




/// @nodoc


class HidlinsApiError_VaultBusySyncing extends HidlinsApiError {
  const HidlinsApiError_VaultBusySyncing(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_VaultBusySyncing);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.vaultBusySyncing()';
}


}




/// @nodoc


class HidlinsApiError_VaultContended extends HidlinsApiError {
  const HidlinsApiError_VaultContended({this.holderPid}): super._();
  

 final  int? holderPid;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_VaultContendedCopyWith<HidlinsApiError_VaultContended> get copyWith => _$HidlinsApiError_VaultContendedCopyWithImpl<HidlinsApiError_VaultContended>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_VaultContended&&(identical(other.holderPid, holderPid) || other.holderPid == holderPid));
}


@override
int get hashCode => Object.hash(runtimeType,holderPid);

@override
String toString() {
  return 'HidlinsApiError.vaultContended(holderPid: $holderPid)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_VaultContendedCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_VaultContendedCopyWith(HidlinsApiError_VaultContended value, $Res Function(HidlinsApiError_VaultContended) _then) = _$HidlinsApiError_VaultContendedCopyWithImpl;
@useResult
$Res call({
 int? holderPid
});




}
/// @nodoc
class _$HidlinsApiError_VaultContendedCopyWithImpl<$Res>
    implements $HidlinsApiError_VaultContendedCopyWith<$Res> {
  _$HidlinsApiError_VaultContendedCopyWithImpl(this._self, this._then);

  final HidlinsApiError_VaultContended _self;
  final $Res Function(HidlinsApiError_VaultContended) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? holderPid = freezed,}) {
  return _then(HidlinsApiError_VaultContended(
holderPid: freezed == holderPid ? _self.holderPid : holderPid // ignore: cast_nullable_to_non_nullable
as int?,
  ));
}


}

/// @nodoc


class HidlinsApiError_PathExists extends HidlinsApiError {
  const HidlinsApiError_PathExists({required this.path}): super._();
  

 final  String path;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_PathExistsCopyWith<HidlinsApiError_PathExists> get copyWith => _$HidlinsApiError_PathExistsCopyWithImpl<HidlinsApiError_PathExists>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_PathExists&&(identical(other.path, path) || other.path == path));
}


@override
int get hashCode => Object.hash(runtimeType,path);

@override
String toString() {
  return 'HidlinsApiError.pathExists(path: $path)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_PathExistsCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_PathExistsCopyWith(HidlinsApiError_PathExists value, $Res Function(HidlinsApiError_PathExists) _then) = _$HidlinsApiError_PathExistsCopyWithImpl;
@useResult
$Res call({
 String path
});




}
/// @nodoc
class _$HidlinsApiError_PathExistsCopyWithImpl<$Res>
    implements $HidlinsApiError_PathExistsCopyWith<$Res> {
  _$HidlinsApiError_PathExistsCopyWithImpl(this._self, this._then);

  final HidlinsApiError_PathExists _self;
  final $Res Function(HidlinsApiError_PathExists) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,}) {
  return _then(HidlinsApiError_PathExists(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HidlinsApiError_FileNotFound extends HidlinsApiError {
  const HidlinsApiError_FileNotFound({required this.path}): super._();
  

 final  String path;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_FileNotFoundCopyWith<HidlinsApiError_FileNotFound> get copyWith => _$HidlinsApiError_FileNotFoundCopyWithImpl<HidlinsApiError_FileNotFound>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_FileNotFound&&(identical(other.path, path) || other.path == path));
}


@override
int get hashCode => Object.hash(runtimeType,path);

@override
String toString() {
  return 'HidlinsApiError.fileNotFound(path: $path)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_FileNotFoundCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_FileNotFoundCopyWith(HidlinsApiError_FileNotFound value, $Res Function(HidlinsApiError_FileNotFound) _then) = _$HidlinsApiError_FileNotFoundCopyWithImpl;
@useResult
$Res call({
 String path
});




}
/// @nodoc
class _$HidlinsApiError_FileNotFoundCopyWithImpl<$Res>
    implements $HidlinsApiError_FileNotFoundCopyWith<$Res> {
  _$HidlinsApiError_FileNotFoundCopyWithImpl(this._self, this._then);

  final HidlinsApiError_FileNotFound _self;
  final $Res Function(HidlinsApiError_FileNotFound) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? path = null,}) {
  return _then(HidlinsApiError_FileNotFound(
path: null == path ? _self.path : path // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HidlinsApiError_KeyfileRequired extends HidlinsApiError {
  const HidlinsApiError_KeyfileRequired(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_KeyfileRequired);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.keyfileRequired()';
}


}




/// @nodoc


class HidlinsApiError_RegistryChanged extends HidlinsApiError {
  const HidlinsApiError_RegistryChanged(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_RegistryChanged);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.registryChanged()';
}


}




/// @nodoc


class HidlinsApiError_InvalidFormat extends HidlinsApiError {
  const HidlinsApiError_InvalidFormat(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_InvalidFormat);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.invalidFormat()';
}


}




/// @nodoc


class HidlinsApiError_RegistryMalformed extends HidlinsApiError {
  const HidlinsApiError_RegistryMalformed(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_RegistryMalformed);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.registryMalformed()';
}


}




/// @nodoc


class HidlinsApiError_SyncNotConfigured extends HidlinsApiError {
  const HidlinsApiError_SyncNotConfigured(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_SyncNotConfigured);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.syncNotConfigured()';
}


}




/// @nodoc


class HidlinsApiError_SyncRemoteUnreachable extends HidlinsApiError {
  const HidlinsApiError_SyncRemoteUnreachable({this.endpoint}): super._();
  

 final  String? endpoint;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_SyncRemoteUnreachableCopyWith<HidlinsApiError_SyncRemoteUnreachable> get copyWith => _$HidlinsApiError_SyncRemoteUnreachableCopyWithImpl<HidlinsApiError_SyncRemoteUnreachable>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_SyncRemoteUnreachable&&(identical(other.endpoint, endpoint) || other.endpoint == endpoint));
}


@override
int get hashCode => Object.hash(runtimeType,endpoint);

@override
String toString() {
  return 'HidlinsApiError.syncRemoteUnreachable(endpoint: $endpoint)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_SyncRemoteUnreachableCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_SyncRemoteUnreachableCopyWith(HidlinsApiError_SyncRemoteUnreachable value, $Res Function(HidlinsApiError_SyncRemoteUnreachable) _then) = _$HidlinsApiError_SyncRemoteUnreachableCopyWithImpl;
@useResult
$Res call({
 String? endpoint
});




}
/// @nodoc
class _$HidlinsApiError_SyncRemoteUnreachableCopyWithImpl<$Res>
    implements $HidlinsApiError_SyncRemoteUnreachableCopyWith<$Res> {
  _$HidlinsApiError_SyncRemoteUnreachableCopyWithImpl(this._self, this._then);

  final HidlinsApiError_SyncRemoteUnreachable _self;
  final $Res Function(HidlinsApiError_SyncRemoteUnreachable) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? endpoint = freezed,}) {
  return _then(HidlinsApiError_SyncRemoteUnreachable(
endpoint: freezed == endpoint ? _self.endpoint : endpoint // ignore: cast_nullable_to_non_nullable
as String?,
  ));
}


}

/// @nodoc


class HidlinsApiError_SyncAuthFailed extends HidlinsApiError {
  const HidlinsApiError_SyncAuthFailed(): super._();
  






@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_SyncAuthFailed);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
  return 'HidlinsApiError.syncAuthFailed()';
}


}




/// @nodoc


class HidlinsApiError_SyncConflictUnresolvable extends HidlinsApiError {
  const HidlinsApiError_SyncConflictUnresolvable({required this.backupPath}): super._();
  

 final  String backupPath;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_SyncConflictUnresolvableCopyWith<HidlinsApiError_SyncConflictUnresolvable> get copyWith => _$HidlinsApiError_SyncConflictUnresolvableCopyWithImpl<HidlinsApiError_SyncConflictUnresolvable>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_SyncConflictUnresolvable&&(identical(other.backupPath, backupPath) || other.backupPath == backupPath));
}


@override
int get hashCode => Object.hash(runtimeType,backupPath);

@override
String toString() {
  return 'HidlinsApiError.syncConflictUnresolvable(backupPath: $backupPath)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_SyncConflictUnresolvableCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_SyncConflictUnresolvableCopyWith(HidlinsApiError_SyncConflictUnresolvable value, $Res Function(HidlinsApiError_SyncConflictUnresolvable) _then) = _$HidlinsApiError_SyncConflictUnresolvableCopyWithImpl;
@useResult
$Res call({
 String backupPath
});




}
/// @nodoc
class _$HidlinsApiError_SyncConflictUnresolvableCopyWithImpl<$Res>
    implements $HidlinsApiError_SyncConflictUnresolvableCopyWith<$Res> {
  _$HidlinsApiError_SyncConflictUnresolvableCopyWithImpl(this._self, this._then);

  final HidlinsApiError_SyncConflictUnresolvable _self;
  final $Res Function(HidlinsApiError_SyncConflictUnresolvable) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? backupPath = null,}) {
  return _then(HidlinsApiError_SyncConflictUnresolvable(
backupPath: null == backupPath ? _self.backupPath : backupPath // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HidlinsApiError_SyncDuplicateTarget extends HidlinsApiError {
  const HidlinsApiError_SyncDuplicateTarget({required this.existingVault}): super._();
  

 final  String existingVault;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_SyncDuplicateTargetCopyWith<HidlinsApiError_SyncDuplicateTarget> get copyWith => _$HidlinsApiError_SyncDuplicateTargetCopyWithImpl<HidlinsApiError_SyncDuplicateTarget>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_SyncDuplicateTarget&&(identical(other.existingVault, existingVault) || other.existingVault == existingVault));
}


@override
int get hashCode => Object.hash(runtimeType,existingVault);

@override
String toString() {
  return 'HidlinsApiError.syncDuplicateTarget(existingVault: $existingVault)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_SyncDuplicateTargetCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_SyncDuplicateTargetCopyWith(HidlinsApiError_SyncDuplicateTarget value, $Res Function(HidlinsApiError_SyncDuplicateTarget) _then) = _$HidlinsApiError_SyncDuplicateTargetCopyWithImpl;
@useResult
$Res call({
 String existingVault
});




}
/// @nodoc
class _$HidlinsApiError_SyncDuplicateTargetCopyWithImpl<$Res>
    implements $HidlinsApiError_SyncDuplicateTargetCopyWith<$Res> {
  _$HidlinsApiError_SyncDuplicateTargetCopyWithImpl(this._self, this._then);

  final HidlinsApiError_SyncDuplicateTarget _self;
  final $Res Function(HidlinsApiError_SyncDuplicateTarget) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? existingVault = null,}) {
  return _then(HidlinsApiError_SyncDuplicateTarget(
existingVault: null == existingVault ? _self.existingVault : existingVault // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HidlinsApiError_InvalidInput extends HidlinsApiError {
  const HidlinsApiError_InvalidInput({required this.field, required this.reason}): super._();
  

 final  String field;
 final  String reason;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_InvalidInputCopyWith<HidlinsApiError_InvalidInput> get copyWith => _$HidlinsApiError_InvalidInputCopyWithImpl<HidlinsApiError_InvalidInput>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_InvalidInput&&(identical(other.field, field) || other.field == field)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode => Object.hash(runtimeType,field,reason);

@override
String toString() {
  return 'HidlinsApiError.invalidInput(field: $field, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_InvalidInputCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_InvalidInputCopyWith(HidlinsApiError_InvalidInput value, $Res Function(HidlinsApiError_InvalidInput) _then) = _$HidlinsApiError_InvalidInputCopyWithImpl;
@useResult
$Res call({
 String field, String reason
});




}
/// @nodoc
class _$HidlinsApiError_InvalidInputCopyWithImpl<$Res>
    implements $HidlinsApiError_InvalidInputCopyWith<$Res> {
  _$HidlinsApiError_InvalidInputCopyWithImpl(this._self, this._then);

  final HidlinsApiError_InvalidInput _self;
  final $Res Function(HidlinsApiError_InvalidInput) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field = null,Object? reason = null,}) {
  return _then(HidlinsApiError_InvalidInput(
field: null == field ? _self.field : field // ignore: cast_nullable_to_non_nullable
as String,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HidlinsApiError_Io extends HidlinsApiError {
  const HidlinsApiError_Io({required this.context}): super._();
  

 final  String context;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_IoCopyWith<HidlinsApiError_Io> get copyWith => _$HidlinsApiError_IoCopyWithImpl<HidlinsApiError_Io>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_Io&&(identical(other.context, context) || other.context == context));
}


@override
int get hashCode => Object.hash(runtimeType,context);

@override
String toString() {
  return 'HidlinsApiError.io(context: $context)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_IoCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_IoCopyWith(HidlinsApiError_Io value, $Res Function(HidlinsApiError_Io) _then) = _$HidlinsApiError_IoCopyWithImpl;
@useResult
$Res call({
 String context
});




}
/// @nodoc
class _$HidlinsApiError_IoCopyWithImpl<$Res>
    implements $HidlinsApiError_IoCopyWith<$Res> {
  _$HidlinsApiError_IoCopyWithImpl(this._self, this._then);

  final HidlinsApiError_Io _self;
  final $Res Function(HidlinsApiError_Io) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? context = null,}) {
  return _then(HidlinsApiError_Io(
context: null == context ? _self.context : context // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class HidlinsApiError_Internal extends HidlinsApiError {
  const HidlinsApiError_Internal({required this.context}): super._();
  

 final  String context;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$HidlinsApiError_InternalCopyWith<HidlinsApiError_Internal> get copyWith => _$HidlinsApiError_InternalCopyWithImpl<HidlinsApiError_Internal>(this, _$identity);



@override
bool operator ==(Object other) {
  return identical(this, other) || (other.runtimeType == runtimeType&&other is HidlinsApiError_Internal&&(identical(other.context, context) || other.context == context));
}


@override
int get hashCode => Object.hash(runtimeType,context);

@override
String toString() {
  return 'HidlinsApiError.internal(context: $context)';
}


}

/// @nodoc
abstract mixin class $HidlinsApiError_InternalCopyWith<$Res> implements $HidlinsApiErrorCopyWith<$Res> {
  factory $HidlinsApiError_InternalCopyWith(HidlinsApiError_Internal value, $Res Function(HidlinsApiError_Internal) _then) = _$HidlinsApiError_InternalCopyWithImpl;
@useResult
$Res call({
 String context
});




}
/// @nodoc
class _$HidlinsApiError_InternalCopyWithImpl<$Res>
    implements $HidlinsApiError_InternalCopyWith<$Res> {
  _$HidlinsApiError_InternalCopyWithImpl(this._self, this._then);

  final HidlinsApiError_Internal _self;
  final $Res Function(HidlinsApiError_Internal) _then;

/// Create a copy of HidlinsApiError
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? context = null,}) {
  return _then(HidlinsApiError_Internal(
context: null == context ? _self.context : context // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
